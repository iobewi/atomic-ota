# FiBeWI ESP32-C3 bootloader

Second-stage Rust `no_std` bootloader owned by FiBeWI.

```text
ESP ROM -> fibewi-esp-bootloader -> ota_0 / ota_1 -> application
```

The executable performs the ESP32-C3 hardware side of the FiBeWI boot contract:

- reads the ESP partition table and EWBT `otadata` entries;
- delegates slot selection, rollback, activation-state transitions and image validation to `fibewi-esp::boot`;
- executes verified `otadata` writes through ROM flash functions;
- copies RAM segments, maps DROM/IROM through the MMU and jumps to the selected application;
- clears the ROM flash-boot watchdog state before handing control to the application.

`fibewi-esp::boot` remains the pure, host-testable decision layer. This crate only executes those decisions on ESP32-C3 hardware.

## Build

```sh
cd bootloader/esp32c3
cargo build --release --locked
```

The workspace is intentionally isolated from the main FiBeWI workspace because the bootloader has its own target, linker script and RAM-only memory layout.

Product repositories own their partition table and factory image packaging; they consume this bootloader but do not carry a copy of its source.
