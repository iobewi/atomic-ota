/*
 * fibewi-esp-bootloader memory layout (ESP32-S3).
 *
 * Mirrors boot.x's (ESP32-C3) approach exactly; only the addresses change,
 * because ESP32-S3's usable internal SRAM is larger. The ROM loads
 * the image found at flash offset 0 -- RAM segments only -- and jumps to its
 * entry point.
 *
 * SRAM is visible through two aliases: instruction bus 0x40370000.. and data
 * bus 0x3fc88000.., offset 0x6f0000 (esp-idf's `SOC_I_D_OFFSET` for
 * ESP32-S3 -- NOT the same constant as ESP32-C3's 0x700000). Code goes in
 * the IRAM alias, data and the stack in the DRAM alias, and the two regions
 * below are chosen so they do NOT cover the same physical bytes:
 *
 *   vectors_seg 0x403b8000..0x403b8400, IRAM 0x403b8400..0x403d0000
 *     <-> DRAM alias 0x3fcc8000..0x3fce0000
 *   DRAM 0x3fce0000..0x3fcec000  (data, bss, stack growing down from the end)
 *
 * DRAM's upper bound is NOT `0x3fd00000` (the chip's real DRAM ceiling): the
 * ROM's own `rom_spiflash_legacy_data` structure -- read and written by the
 * ROM SPI flash driver `espbewi-boot` calls into (`set_flash_size_from_header`
 * writes `rom_spiflash_legacy_data[1]` directly) -- is PROVIDEd by
 * `esp-rom-sys`'s `esp32s3.rom.ld` at the fixed address `0x3fceffe4`, a mere
 * 28 bytes below that ceiling. A first ESP32-S3 boot with DRAM sized
 * `0x3fce0000..0x3fd00000` put the stack (which starts at the region's top
 * and grows down) directly on top of that address: any call depth past 28
 * bytes of stack already collided with it, and the ROM's own write to its
 * struct then landed on live stack content. Symptom on real hardware: the
 * very first ROM flash call corrupts state and output turns to garbage
 * immediately after (no clean panic -- a live stack overwritten mid-flight
 * doesn't unwind cleanly). ESP32-C3's `boot.x` never had this problem only
 * because its DRAM window (`0x3fcd4000..0x3fcdc000`) already happened to end
 * a safe `0x3ff0` bytes short of *its* chip's equivalent address
 * (`0x3fcdfff0`) -- not because the hazard doesn't exist there too, but
 * because nobody had to reason about it explicitly until the S3 port made it
 * fatal. This window keeps the same margin below `0x3fceffe4` (`0x3fe4`
 * bytes) that C3's already had below its own address, rather than reaching
 * for the chip's true ceiling.
 *
 * Both windows sit inside the top of SRAM, below `boot_window`'s upper bound
 * (0x3fd00000, the top of ESP32-S3's DRAM range) -- that whole span is what
 * `espbewi_platform::chips::esp32s3::BOOT_MEMORY_MAP.boot_window` reserves,
 * so it must stay consistent with this file if either one moves.
 *
 * The application image is loaded at the bottom of SRAM (0x40370000 /
 * 0x3fc88000 upwards); `main.rs` refuses any segment reaching this window.
 */
MEMORY
{
  /* Xtensa's vector table (xtensa-lx-rt's `exception.x`) is a fixed 0x400-byte,
   * hardware-aligned block that must land in a region literally named
   * `vectors_seg`; carved off the front of the same IRAM window so the vector
   * table and the naked exception handlers it jumps to (call0, short range)
   * stay adjacent. */
  vectors_seg (RX)  : ORIGIN = 0x403b8000, LENGTH = 0x400
  IRAM        (RWX) : ORIGIN = 0x403b8400, LENGTH = 0x18000 - 0x400
  DRAM        (RW)  : ORIGIN = 0x3fce0000, LENGTH = 0xC000
  RTC_FAST    (RWX) : ORIGIN = 0x600fe000, LENGTH = 0x2000
  /* ESP32-S3's second, larger RTC bank (unlike ESP32-C3, which has only the
   * one above). Declared -- `rtc_slow.x`'s sections need a real region of
   * this exact name to place their (here, empty) output into -- but not
   * used for anything: nothing in this bootloader targets `#[ram(rtc_slow)]`. */
  rtc_slow_seg (RWX) : ORIGIN = 0x50000000, LENGTH = 0x2000
}

/* esp-rom-sys 0.1.4/0.1.5's ESP32-S3 `additional.ld` provides this ROM
 * function only as `rom_Cache_Suspend_ICache`, not the plain name
 * `espbewi_boot::esp32s3::hw` (and the ESP32-C3 module, and every other ROM
 * function on both chips) calls directly. Address confirmed against a newer
 * esp-hal revision's `esp32s3.rom.ld`, which does provide the plain name at
 * the same address -- this is a naming gap in the pinned crate version, not
 * an uncertain address. */
PROVIDE(Cache_Suspend_ICache = 0x4000189c);

/* esp-hal's section files place things by these logical names; point all of
 * them at RAM (rodata goes to the *data* alias: byte loads from the
 * instruction alias are not safe). */
REGION_ALIAS("ROTEXT", IRAM);
REGION_ALIAS("RODATA", DRAM);
REGION_ALIAS("RWDATA", DRAM);
REGION_ALIAS("RWTEXT", IRAM);
REGION_ALIAS("RTC_FAST_RWTEXT", RTC_FAST);
REGION_ALIAS("RTC_FAST_RWDATA", RTC_FAST);
REGION_ALIAS("RTC_SLOW_RWTEXT", rtc_slow_seg);
REGION_ALIAS("RTC_SLOW_RWDATA", rtc_slow_seg);

/* Xtensa needs its own vector table; RISC-V's script has no equivalent of
 * this include. Provided by xtensa-lx-rt, placed like esp-hal's own
 * esp32s3.x does it. */
INCLUDE exception.x

/* No flash-mapped sections, hence no `.rotext_dummy`/`.rwdata_dummy` tricks
 * (esp32s3.x): with disjoint IRAM/DRAM windows nothing overlaps. */
SECTIONS {
  INCLUDE "rwtext.x"
  INCLUDE "rwdata.x"
}
INCLUDE "rodata.x"
INCLUDE "text.x"
INCLUDE "rtc_fast.x"
/* xtensa-lx-rt's Reset handler unconditionally zeroes/initializes rtc_slow's
 * bss/persistent regions regardless of whether anything is placed there;
 * include it (onto the RTC_SLOW_* alias above) purely so those symbols
 * exist, not because this bootloader uses rtc_slow for anything. */
INCLUDE "rtc_slow.x"
INCLUDE "stack.x"
INCLUDE "metadata.x"
INCLUDE "eh_frame.x"

INCLUDE "hal-defaults.x"
