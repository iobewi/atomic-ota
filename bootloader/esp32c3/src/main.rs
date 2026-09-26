//! embewi-boot -- second-stage bootloader for the ESP32-C3, in Rust.
//!
//!   ROM -> embewi-boot -> the slot `otadata` selects -> embewi-agent
//!
//! Every *decision* -- which slot, what to write to `otadata`, whether an image
//! is bootable -- is `fibewi-esp::boot`, tested on the host against power cuts.
//! This file only performs them on the real flash and hands over control.
//!
//! What it does today (boot chain step 5):
//! * clears the ROM's "flash boot" watchdog protection (see below);
//! * reads the partition table, then the two `otadata` entries;
//! * asks `plan_boot` what to do. A blank `otadata` is a first boot: slot 0 is
//!   validated, then `Valid(seq=1)` is written by the protocol below, and only
//!   then does it boot. Anything unexpected halts -- it never guesses;
//! * loads the chosen image (RAM segments copied, DROM/IROM mapped through the
//!   flash MMU) and jumps.
//!
//! Not yet: checksum/SHA verification (image checks stop at `Verify::Structure`),
//! the watchdog handover a hung image needs, and an agent that writes the entry
//! format this reads -- until then only the bootloader creates entries.
//!
//! Writing one `otadata` entry (`Write::ops`), each step verified before the next:
//!
//! ```text
//! erase sector      -> read back: erased
//! program the body  -> read back: exactly the body, commit word still erased
//! program the commit word (a separate flash command)
//!                   -> read back: exactly the committed entry, and it decodes
//! ```
//!
//! The commit word therefore means "I checked this exact body", not merely "a
//! second command ran". Any failed step halts.
#![no_std]
#![no_main]

#[cfg(not(any(feature = "esp32c3", feature = "esp32s3")))]
compile_error!("select a supported ESP boot target feature");
#[cfg(all(feature = "esp32c3", feature = "esp32s3"))]
compile_error!("select exactly one ESP boot target feature");

use fibewi_esp::boot as boot_core;

use boot_core::image::{self, Verify};
use boot_core::{BLANK, Boot, Decoded, ENTRY_SIZE, Halt, Op, Raw, Write, decode, plan_boot};
use esp_println::Printer;
#[cfg(feature = "esp32c3")]
use espbewi_boot::esp32c3::hw;
#[cfg(feature = "esp32s3")]
use espbewi_boot::esp32s3::hw;

/// What `log!` can print. Text and hex only, on purpose: `core::fmt` (Debug,
/// padding, Unicode tables) costs ~10 KiB, and the whole bootloader has to
/// fit in the 32 KiB in front of the partition table.
trait Loggable {
    fn put(&self);
}

impl Loggable for &str {
    fn put(&self) {
        Printer::write_bytes(self.as_bytes());
    }
}

/// Printed as `0x` + 8 hex digits.
impl Loggable for u32 {
    fn put(&self) {
        let mut out = *b"0x00000000";
        for i in 0..8 {
            let nibble = ((*self >> ((7 - i) * 4)) & 0xF) as u8;
            out[2 + i] = if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 };
        }
        Printer::write_bytes(&out);
    }
}

macro_rules! log {
    ($($part:expr),+ $(,)?) => {{
        $( Loggable::put(&$part); )+
        Printer::write_bytes(b"\r\n");
    }};
}

// --- Flash layout (embewi-ab-v1, see partitions.csv) -----------------------

/// Where the partition table lives (the ESP-IDF default, kept by espflash).
const PARTITION_TABLE_OFFSET: u32 = 0x8000;
const PARTITION_TABLE_LEN: usize = 0xC00;
/// Stored as the bytes `AA 50`, hence 0x50AA once read little-endian.
const PARTITION_ENTRY_MAGIC: u16 = 0x50AA;
const TYPE_APP: u8 = 0x00;
const TYPE_DATA: u8 = 0x01;
const SUBTYPE_OTA_0: u8 = 0x10;
const SUBTYPE_OTA_1: u8 = 0x11;
const SUBTYPE_OTADATA: u8 = 0x00;
const SECTOR: u32 = 0x1000;
const SLOT_COUNT: u8 = 2;

#[cfg(feature = "esp32c3")]
const MAP: espbewi_platform::MemoryMap = espbewi_platform::chips::esp32c3::BOOT_MEMORY_MAP;
#[cfg(feature = "esp32s3")]
const MAP: espbewi_platform::MemoryMap = espbewi_platform::chips::esp32s3::BOOT_MEMORY_MAP;

// ROM/MMU/watchdog access is provided by espbewi-boot, per chip.

enum BootError {
    FlashRead(u32),
    NoPartitionTable,
    /// A partition the layout requires is missing: 1 = otadata, 2 = ota_0, 3 = ota_1.
    MissingPartition(u32),
    /// `plan_boot` says there is nothing safe to boot: 1 = first boot, slot 0 unbootable; 2 = no usable entry.
    Halt(u32),
    /// An `otadata` write step failed verification: 0 erase, 1 body, 2 commit.
    OtadataWrite { step: u32, at: u32 },
    /// The chosen image failed validation when re-read to be loaded.
    Image,
    Mmu(i32),
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Reads `out.len()` bytes at flash offset `offset`. The ROM reader wants
/// word-aligned offsets, buffers and lengths, so this reads aligned windows
/// and copies the requested bytes out of them.
fn flash_read(offset: u32, out: &mut [u8]) -> Result<(), BootError> {
    hw::flash_read(offset, out).map_err(|e| match e {
        hw::FlashError::Read(at) => BootError::FlashRead(at),
        _ => BootError::FlashRead(offset),
    })
}

/// The image validator reads through this.
struct RomFlash;

impl image::Read for RomFlash {
    fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), ()> {
        flash_read(offset, buf).map_err(|_| ())
    }
}

/// Tell the ROM flash driver how large the physical flash is.
fn set_flash_size() -> Result<(), BootError> {
    let size = hw::set_flash_size_from_header().map_err(|e| match e {
        hw::FlashError::Read(at) => BootError::FlashRead(at),
        _ => BootError::FlashRead(0),
    })?;
    log!("boot: flash size ", size);
    Ok(())
}

/// Re-attach/configure flash after an early ROM read failure.
fn flash_reinit() {
    let chip_size = hw::flash_reinit();
    log!("boot: re-attaching flash, chip_size=", chip_size);
}

/// The three partitions the A/B layout needs: (offset, size) of each.
struct Layout {
    otadata: (u32, u32),
    apps: [(u32, u32); SLOT_COUNT as usize],
}

fn read_layout() -> Result<Layout, BootError> {
    let mut table = [0u8; PARTITION_TABLE_LEN];
    if flash_read(PARTITION_TABLE_OFFSET, &mut table).is_err() {
        flash_reinit();
        flash_read(PARTITION_TABLE_OFFSET, &mut table)?;
    }
    if u16::from_le_bytes([table[0], table[1]]) != PARTITION_ENTRY_MAGIC {
        return Err(BootError::NoPartitionTable);
    }
    let (mut otadata, mut ota_0, mut ota_1) = (None, None, None);
    for entry in table.chunks_exact(32) {
        if u16::from_le_bytes([entry[0], entry[1]]) != PARTITION_ENTRY_MAGIC {
            break; // 0xEBEB (MD5 marker) or erased flash: end of the entries
        }
        let found = Some((le32(entry, 4), le32(entry, 8)));
        match (entry[2], entry[3]) {
            (TYPE_DATA, SUBTYPE_OTADATA) => otadata = found,
            (TYPE_APP, SUBTYPE_OTA_0) => ota_0 = found,
            (TYPE_APP, SUBTYPE_OTA_1) => ota_1 = found,
            _ => {}
        }
    }
    let otadata = otadata.ok_or(BootError::MissingPartition(1))?;
    if otadata.1 < 2 * SECTOR {
        return Err(BootError::MissingPartition(1));
    }
    Ok(Layout {
        otadata,
        apps: [ota_0.ok_or(BootError::MissingPartition(2))?, ota_1.ok_or(BootError::MissingPartition(3))?],
    })
}

fn read_otadata(layout: &Layout) -> Result<[Raw; 2], BootError> {
    let mut entries = [BLANK; 2];
    for (i, raw) in entries.iter_mut().enumerate() {
        flash_read(layout.otadata.0 + i as u32 * SECTOR, raw)?;
    }
    Ok(entries)
}

/// Programs `data` (a multiple of 4 bytes, at a 4-byte-aligned address) through the ROM.
fn rom_program(at: u32, data: &[u8]) -> bool {
    hw::flash_program(at, data).is_ok()
}

/// Performs one `otadata` entry update with a read-back after every command.
fn execute(write: Write, layout: &Layout) -> Result<(), BootError> {
    let base = layout.otadata.0 + u32::from(write.sector) * SECTOR;
    let fail = |step: u32| BootError::OtadataWrite { step, at: base };
    let [erase, body, commit] = write.ops();
    let mut back = [0u8; ENTRY_SIZE];

    if hw::flash_unlock().is_err() {
        return Err(fail(0));
    }

    // 0. erase, and see it erased
    if !matches!(erase, Op::Erase { .. }) || hw::flash_erase_sector(base / SECTOR).is_err() {
        return Err(fail(0));
    }
    flash_read(base, &mut back)?;
    if back != BLANK {
        return Err(fail(0));
    }

    // 1. the body, and see exactly the body (commit word still erased)
    let Op::Program { offset, len, data, .. } = body else { return Err(fail(1)) };
    if !rom_program(base + u32::from(offset), &data[..usize::from(len)]) {
        return Err(fail(1));
    }
    flash_read(base, &mut back)?;
    if back != write.entry.body() {
        return Err(fail(1));
    }

    // 2. the commit word, on its own; then the whole entry must read back committed and exact
    let Op::Program { offset, len, data, .. } = commit else { return Err(fail(2)) };
    if !rom_program(base + u32::from(offset), &data[..usize::from(len)]) {
        return Err(fail(2));
    }
    flash_read(base, &mut back)?;
    if back != write.entry.encode() || decode(&back) != Decoded::Ok(write.entry) {
        return Err(fail(2));
    }
    Ok(())
}

fn boot() -> Result<core::convert::Infallible, BootError> {
    set_flash_size()?;
    let layout = read_layout()?;
    log!("boot: otadata at ", layout.otadata.0, " ota_0 ", layout.apps[0].0, " ota_1 ", layout.apps[1].0);
    let entries = read_otadata(&layout)?;

    let mut image_ok = |slot: u8| {
        let (offset, size) = layout.apps[usize::from(slot)];
        match image::validate(&mut RomFlash, offset, size, &MAP, Verify::Structure) {
            Ok(_) => true,
            Err(e) => {
                log!("boot: slot ", u32::from(slot), " image refused, reason ", image_error_code(&e));
                false
            }
        }
    };
    let plan = plan_boot(entries, SLOT_COUNT, &mut image_ok);

    let (slot, seq) = match plan.boot {
        Boot::Slot { slot, seq, .. } => (slot, seq),
        Boot::Halt(Halt::NoImage) => return Err(BootError::Halt(1)),
        Boot::Halt(Halt::NoUsableEntry) => return Err(BootError::Halt(2)),
    };
    log!("boot: plan slot=", u32::from(slot), " seq=", seq);

    for write in plan.writes() {
        log!(
            "boot: otadata write sector=",
            u32::from(write.sector),
            " seq=",
            write.entry.seq,
            " state=",
            write.entry.state
        );
        execute(write, &layout)?;
    }

    load(&layout, slot)
}

/// Loads slot `slot`'s image and jumps to it.
fn load(layout: &Layout, slot: u8) -> Result<core::convert::Infallible, BootError> {
    let (offset, size) = layout.apps[usize::from(slot)];
    let image = image::validate(&mut RomFlash, offset, size, &MAP, Verify::Structure).map_err(|_| BootError::Image)?;
    log!("boot: image ok, segments=", image.count as u32, " entry=", image.entry);

    // Copy the RAM segments (validated: inside the map, clear of this bootloader).
    for seg in image.segments().iter().filter(|s| s.len > 0) {
        let Some(target) = seg.ram_target(&MAP) else { continue };
        let dst = target as *mut u8;
        let mut chunk = [0u8; 256];
        let mut copied = 0u32;
        while copied < seg.len {
            let n = (seg.len - copied).min(chunk.len() as u32) as usize;
            flash_read(seg.data_offset + copied, &mut chunk[..n])?;
            unsafe { core::ptr::copy_nonoverlapping(chunk.as_ptr(), dst.add(copied as usize), n) };
            copied += n as u32;
        }
    }

    // Flash MMU + cache for the DROM/IROM segments.
    let autoload = hw::cache_begin_mapping();
    for seg in image.segments().iter().filter(|s| s.len > 0) {
        let is_drom = MAP.drom.contains(&seg.load);
        if !(is_drom || MAP.irom.contains(&seg.load)) {
            continue;
        }
        let vaddr = seg.load & !(MAP.mmu_page - 1);
        let paddr = seg.data_offset & !(MAP.mmu_page - 1);
        let pages = (seg.len + (seg.load - vaddr)).div_ceil(MAP.mmu_page);
        let rc = if is_drom {
            hw::map_drom(vaddr, paddr, pages)
        } else {
            hw::map_irom(vaddr, paddr, pages)
        };
        log!("boot: map ", vaddr, " <- flash ", paddr, " pages=", pages, " rc=", rc as u32);
        if rc != 0 {
            return Err(BootError::Mmu(rc));
        }
    }
    hw::cache_finish_mapping(autoload);

    log!("boot: jump ", image.entry);
    // Let the USB-Serial-JTAG FIFO drain before the application reconfigures it.
    hw::delay_us(50_000);
    let entry: extern "C" fn() -> ! = unsafe { core::mem::transmute(image.entry as usize) };
    entry()
}

/// A stable number per refusal reason (no `Debug`: it costs code size).
fn image_error_code(e: &image::ImageError) -> u32 {
    use image::ImageError::*;
    match e {
        Read => 1,
        BadMagic(_) => 2,
        BadChip(_) => 3,
        BadSegmentCount(_) => 4,
        SegmentOutsidePartition(_) => 5,
        BadLoadRange(_) => 6,
        Misaligned(_) => 7,
        OverlapsBootloader(_) => 8,
        BadEntry => 9,
        Truncated => 10,
        BadChecksum => 11,
        HashMissing => 12,
        BadHash => 13,
    }
}

/// One line per failure, with the offending value where there is one.
fn report(e: &BootError) {
    match e {
        BootError::FlashRead(at) => log!("boot: FAILED flash read at ", *at),
        BootError::NoPartitionTable => log!("boot: FAILED no partition table"),
        BootError::MissingPartition(which) => log!("boot: FAILED missing partition ", *which),
        BootError::Halt(why) => log!("boot: HALT, nothing safe to boot, reason ", *why),
        BootError::OtadataWrite { step, at } => log!("boot: FAILED otadata write step ", *step, " at ", *at),
        BootError::Image => log!("boot: FAILED image no longer validates"),
        BootError::Mmu(rc) => log!("boot: FAILED MMU rc ", *rc as u32),
    }
}

/// Clear the ROM flash-boot watchdog mode before handing control to the application.
fn clear_flashboot_watchdogs() -> (u32, u32) {
    hw::clear_flashboot_watchdogs()
}

#[esp_hal::main]
fn main() -> ! {
    // DIAGNOSTIC (ESP32-S3 bring-up): the very first log line after
    // esp_hal::init() (below) never appeared on real hardware even though
    // the panic handler's own later output does -- bracketing checkpoints,
    // not relying on PanicInfo::location() (which itself printed an empty
    // file + line 0, uninformative), to find out how far execution actually
    // gets before whatever crashes.
    log!("\r\ncheckpoint 0: main() entered");
    // First thing: the ROM's watchdogs are already ticking.
    let (tg0_wdt, rtc_wdt) = clear_flashboot_watchdogs();
    log!("checkpoint 1: flashboot watchdogs cleared");
    esp_hal::init(esp_hal::Config::default());
    log!("checkpoint 2: esp_hal::init() returned");
    log!("\r\nembewi-boot ", env!("CARGO_PKG_VERSION"), " (otadata bootstrap; no rollback yet)");
    log!("boot: wdt tg0=", tg0_wdt, " rtc=", rtc_wdt, " (flashboot bits cleared)");
    // `boot` only ever returns on failure (success ends in a jump).
    let Err(e) = boot();
    report(&e);
    // Nothing safe left to do; stay put so the message can be read.
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // DIAGNOSTIC (ESP32-S3 bring-up): file+line only, via the existing
    // `Loggable` impls for `&str`/`u32` -- no `core::fmt` formatting of the
    // panic message itself, so this stays inside the "text and hex only"
    // code-size budget `log!` was built for.
    if let Some(loc) = info.location() {
        log!("boot: PANIC at ", loc.file(), ":", loc.line());
    } else {
        log!("boot: PANIC");
    }
    loop {
        core::hint::spin_loop();
    }
}
