# FiBeWI

FiBeWI is a `no_std` firmware lifecycle engine focused on transactional,
resumable updates and restart-safe A/B activation.

The repository is a workspace with a platform-independent core and platform
backends:

```text
fibewi/
├── src/                  # generic firmware-update core
└── backends/
    └── esp/              # ESP flash, partitions, EWBT/otadata and image validation
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
bootloader formats, or application configuration.

## Backend `fibewi-esp`

`backends/esp` owns ESP-specific firmware mechanics:

- ESP-IDF `ota_0` / `ota_1` partition discovery;
- NOR-flash erase/program mechanics for FiBeWI artifacts;
- the EWBT transactional `otadata` format and its power-cut-safe state machine;
- ESP application-image structural/checksum/SHA-256 validation.

The update transaction state and the boot trust state remain separate state
machines even though they now live in the same repository.

The ESP backend still does not own application transaction metadata, NVS
configuration, HTTP/TLS, or deployment policy.

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
`atomic-ota-esp` responsibilities into one repository while keeping the
generic/platform boundary explicit.

## License

MIT.
