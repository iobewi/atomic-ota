//! A transactional, resumable, A/B-safe OTA engine core.
//!
//! `no_std`, and independent of the ESP32, `esp-hal`, Embassy, HTTP/TLS,
//! Kubernetes/any particular deployment control plane, and any one
//! bootloader's on-flash format.
//!
//! # Stability
//!
//! Nothing in this crate is a stable API yet: every public enum is
//! `#[non_exhaustive]` and every signature should be expected to move as
//! the multi-artifact and rollback paths are built out on top of it.
#![no_std]

extern crate alloc;

pub mod artifact;
pub mod error;
pub mod state;
pub mod storage;

pub use artifact::{Committed, Digest, ResumePlan, WriteSession, is_complete, resume_plan};
pub use error::Error;
pub use state::{Action, BackendOutcome, TransactionState};
pub use storage::{ArtifactStorage, TransactionMetadata};
