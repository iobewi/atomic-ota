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

- mapping FiBeWI `ota_0` / `ota_1` slots onto partitions located by `espbewi`;
- FiBeWI artifact buffering/writes over the common ESP raw-storage primitives;
- the EWBT transactional `otadata` format and its power-cut-safe state machine;
- ESP application-image structural/checksum/SHA-256 validation.

The update transaction state and the boot trust state remain separate state
machines even though they now live in the same repository.

The ESP backend still does not own the physical flash capability, generic ESP
partition-table/raw erase primitives, application transaction metadata, NVS
configuration, HTTP/TLS, or deployment policy. The common hardware storage
layer is `espbewi`.

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

## ESP second-stage bootloader

`bootloader/esp32c3` contains the FiBeWI-owned ESP32-C3 second-stage bootloader.
It consumes the pure `fibewi-esp::boot` decision/image-validation core and performs
the ROM flash, MMU/cache, RAM-load and final jump operations on hardware.

Product repositories provide their partition table and factory packaging; they do
not own a separate boot state machine or bootloader implementation.
