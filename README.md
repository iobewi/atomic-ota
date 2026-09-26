# FiBeWI

FiBeWI is a `no_std` firmware lifecycle engine focused on transactional,
resumable updates and restart-safe A/B activation.

The repository is a workspace with a platform-independent core and platform
backends:

```text
fibewi/
├── src/                  # generic firmware-update core
└── backends/
    └── esp/              # ESP slot mapping, EWBT/otadata and image validation
```

## Core `fibewi`

The root crate owns:

- resumable streaming writes and durable-progress tracking;
- SHA-256 verification while bytes become durable;
- staged transaction records;
- `Staged -> Activating -> resolved` transaction semantics;
- deterministic reconciliation after restart;
- backend contracts for artifact storage and transaction metadata.

It deliberately does not know about HTTP, TLS, NVS, ESP partition tables,
bootloader executables, linker layouts, MMU/cache programming, watchdog
registers, or application configuration.

## Backend `fibewi-esp`

`backends/esp` owns ESP-specific firmware lifecycle semantics:

- mapping FiBeWI `ota_0` / `ota_1` slots onto partitions located by `espbewi`;
- FiBeWI artifact buffering/writes over the common ESP raw-storage primitives;
- the EWBT transactional `otadata` format and its power-cut-safe state machine;
- ESP application-image structural/checksum/SHA-256 validation;
- the pure boot decision layer consumed by platform bootloader executors.

The update transaction state and the boot trust state remain separate state
machines even though they live in the same repository.

The ESP backend does not own the physical flash capability, generic ESP
partition-table/raw erase primitives, NVS configuration, ROM flash calls,
MMU/cache setup, watchdog handling, linker scripts, or the second-stage
bootloader executable. Those hardware execution responsibilities belong to
`espbewi`.

## Bootloader ownership

FiBeWI intentionally contains no ESP bootloader executable.

The ESP second-stage bootloader is owned by
[`espbewi/bootloader/esp`](https://github.com/iobewi/espbewi/tree/refactor/bootloader-owner/bootloader/esp).
It depends on FiBeWI for EWBT/A-B lifecycle policy and image-validation
semantics, while `espbewi` owns the hardware execution boundary.

```text
ESP ROM
  -> espbewi-bootloader
       -> espbewi hardware/platform
       -> fibewi-esp::boot semantics
  -> application
```

## Tests

Host tests:

```sh
cargo test -p fibewi
cargo test -p fibewi-esp
```

ESP backend compile gate:

```sh
cargo check -p fibewi-esp --features esp32c3 --target riscv32imc-unknown-none-elf
```

The `fibewi-esp` host suite includes the adversarial EWBT power-cut model
imported from the former `atomic-boot` repository.

## Status

Pre-stable. FiBeWI consolidates the former `atomic-ota`, `atomic-boot`, and
`atomic-ota-esp` lifecycle responsibilities while keeping hardware execution
outside the repository.

## License

MIT.
