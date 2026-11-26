//! PEAK PCAN-Basic backend for the [`canbus`](https://docs.rs/canbus) crate
//! family.
//!
//! Applications normally use this through `canbus::Bus` with interface name
//! `"pcan"`; depend on this crate directly only to skip the facade.
//!
//! PCAN-Basic (`PCANBasic.dll` on Windows, `libpcanbasic.so` on Linux) is
//! loaded at runtime the first time a bus is opened. Set `PCAN_SDK_DIR` to
//! load it from a non-standard location. On platforms PEAK doesn't support,
//! opening fails with
//! [`Error::LibraryNotLoaded`](canbus_core::Error::LibraryNotLoaded).
//!
//! # Configuration
//!
//! - **Channel:** a PCAN-Basic channel name — `PCAN_USBBUS1`..`16`,
//!   `PCAN_PCIBUS1`..`16`, `PCAN_LANBUS1`..`16` (case-insensitive) — or a
//!   raw handle such as `0x51`. Use [`detect`] to list channels.
//! - **Bitrate (required):** classic CAN uses PCAN's presets (1M, 800k, 500k,
//!   250k, 125k, 100k, 95k, 83k, 50k, 47k, 33k, 20k, 10k, 5k). Explicit
//!   [`Timing`](canbus_core::Timing) must use the 8 MHz SJA1000 clock.
//! - **CAN FD:** nominal 125k/250k/500k/1M and data 500k/1M/2M/4M/8M have
//!   80 MHz presets; otherwise give explicit nominal and data timing with the
//!   same clock.
//! - **Listen-only:** sets `PCAN_LISTEN_ONLY` before initialisation.
//!
//! Timestamps have microsecond resolution, counted from when the driver
//! started. `flush_tx` is not supported: PCAN-Basic's only flush also
//! discards received frames.

mod bus;
mod channel_name;
mod convert;
mod event;
mod ffi;

use canbus_core::{BusConfig, CanBus, Interface, Result};

pub use bus::{detect, PcanBus};

pub(crate) const NAME: &str = "pcan";

/// Descriptor for the PCAN backend.
pub const INTERFACE: Interface = Interface::new(NAME, "PEAK PCAN-Basic", open, detect);

fn open(config: &BusConfig) -> Result<Box<dyn CanBus>> {
    Ok(Box::new(PcanBus::open(config)?))
}
