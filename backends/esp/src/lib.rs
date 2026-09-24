#![cfg_attr(not(test), no_std)]

//! ESP backend for FiBeWI.
//!
//! The crate contains the ESP-specific half of the firmware lifecycle:
//!
//! - ESP-IDF OTA partition lookup and NOR-flash artifact writes;
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
