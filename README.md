# atomic-ota

A transactional, resumable, A/B-safe OTA engine core. `no_std` (+ `alloc`),
and independent of any MCU, HAL, async runtime, transport protocol,
deployment control plane, or bootloader's on-flash format.

```
cargo test
```

runs the whole suite on a plain host toolchain -- no ESP-IDF, no `no_std`
target, no hardware.

## Status

Extraction in progress from
[`embewi-agent-esp`](https://github.com/iobewi/embewi-agent-esp), the first
real user of this engine. The pure core (this repo) is implemented and
tested; ESP32 reintegration (a backend adapter living in
`embewi-agent-esp`, implementing the traits below against `esp-storage`
and NVS) has not happened yet. Nothing here is a stable API: every public
enum is `#[non_exhaustive]`.

## What this crate owns

- **Streaming, resumable artifact writes** ([`artifact`]) -- accepts bytes
  in any chunking, tracks *received* (accepted into the session)
  separately from *durable* (what the backend has confirmed), and hashes
  exactly the durable bytes as they become durable. Never a post-hoc
  re-read to compute or check a digest.
- **A staged-transaction record and its lifecycle** ([`transaction`]) --
  `Staged -> Activating -> (resolved)`, with an artifact list shaped for
  multiple artifacts per transaction from day one, even though v1 only
  ever stages one.
- **Deterministic post-restart reconciliation** ([`transaction::reconcile`])
  -- given what was staged, what the backend now reports, and whether the
  currently-selected target matches what was staged, decide the one right
  action. A restart at any point during a write, an activation, or a
  confirmation lands on a decision this table has already accounted for.
- **Two backend boundaries** ([`storage`]), deliberately not one:
  - `ArtifactStorage` -- bulk bytes, not read-back-verified by this crate,
    no notion of sectors/alignment/padding. The backend reports a
    durability watermark; how it gets there is its own business.
  - `TransactionMetadata` -- a small record whose one write operation
    (`commit`) is the one primitive this crate trusts to be genuinely
    indivisible. This crate does not compose that guarantee out of weaker
    primitives on a backend's behalf -- a backend that can only offer
    ordered multi-write-with-a-marker, or
    erase/body/readback/commit/readback, has to provide it itself, behind
    one `commit` call.

## What this crate deliberately does not own

- Any resume-token wire format (`Content-Range` or otherwise) --
  `artifact::resume_plan`/`artifact::is_complete` take decoded numbers;
  parsing a header into them is transport-layer, outside this crate.
- Flash mechanics -- sectors, erase/program, word alignment, padding --
  all backend detail behind `ArtifactStorage`.
- Any one A/B slot-trust encoding (EWBT's commit-word `otadata` format, or
  anyone else's) -- this crate consumes only the coarser fact
  `state::BackendOutcome` describes: is there an unconfirmed candidate, is
  the current state confirmed. The format that produces that fact, and the
  bootloader that acts on it, live below this crate.
- HTTP, TLS, auth, JSON -- none of it. A caller's protocol layer translates
  its wire format into calls against this crate's API and back.
- OCI, secure boot, any particular partition scheme, any particular MCU.

## Two atomicities, not one

A real device already relies on two different guarantees that must not be
confused with each other:

- the **artifact payload** is written once, streamed, and never read back
  to verify it -- its digest is accumulated while writing;
- the **transaction metadata** (a few dozen bytes: which artifact, which
  target, what state) is small enough to demand a real, single, atomic
  publish, verified by the backend before it is trusted.

Modeling both as "just write some bytes durably" would let a caller mistake
"several writes succeeded" for "the transaction is atomically published" --
exactly the confusion `ArtifactStorage` vs. `TransactionMetadata` keeps
apart.

## License

MIT.
