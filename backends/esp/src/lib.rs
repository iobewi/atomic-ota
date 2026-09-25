#![cfg_attr(not(test), no_std)]

//! ESP backend for FiBeWI.
//!
//! The crate contains the ESP-specific half of the firmware lifecycle:
//!
//! - FiBeWI OTA-slot mapping and artifact writes over esp-storage-manager;
//! - EWBT/otadata transactional A/B boot state;
//! - ESP application-image validation.
//!
//! Transaction identity, staged metadata persistence, HTTP and application
//! policy remain outside this backend.

pub mod boot;

#[cfg(any(feature = "esp32c3", feature = "esp32s3"))]
mod artifact;
#[cfg(any(feature = "esp32c3", feature = "esp32s3"))]
pub use artifact::*;
