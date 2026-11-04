//! MTP (Media Transfer Protocol) responder for a single storage.
//!
//! The responder owns all protocol logic and talks to the outside world
//! through two traits: [`Transport`] for bulk packets and [`Storage`] for the
//! filesystem. Both can be faked, so the protocol is tested on the host.
#![no_std]

pub mod codes;
pub mod container;
pub mod dataset;
pub mod handles;
mod responder;
pub mod storage;
pub mod transport;
#[cfg(feature = "usb")]
pub mod usb;

pub use dataset::{MAX_NAME, Name};
pub use responder::{DeviceStrings, MAX_PATH, PathBuf, Responder, State};
pub use storage::{EntryMeta, Storage, StorageError};
pub use transport::Transport;

#[cfg(test)]
mod tests;
