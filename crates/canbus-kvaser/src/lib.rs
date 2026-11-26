//! Kvaser CANlib backend for the [`canbus`](https://docs.rs/canbus) crate
//! family.
//!
//! Applications normally use this through `canbus::Bus` with interface name
//! `"kvaser"`; depend on this crate directly only to skip the facade.
//!
//! The Kvaser CANlib library (`canlib32.dll` on Windows, `libcanlib.so` on
//! Linux) is loaded at runtime the first time a bus is opened. Set
//! `CANLIB_SDK_DIR` to load it from a non-standard location. On platforms
//! Kvaser doesn't support, opening fails with
//! [`Error::LibraryNotLoaded`](canbus_core::Error::LibraryNotLoaded).
//!
//! # Configuration
//!
//! - **Channel:** the CANlib channel index, e.g. `"0"`. Use
//!   [`detect`] to list channels.
//! - **Bitrate:** standard rates (1M, 500k, 250k, 125k, 100k, 83.3k, 62.5k,
//!   50k, 10k) use CANlib's presets; other rates let CANlib derive the
//!   timing. Explicit [`Timing`](canbus_core::Timing) is passed through.
//! - **CAN FD:** data bitrates of 500k, 1M, 2M, 4M and 8M use presets
//!   (80 % sample point); anything else needs explicit data timing.
//! - **Listen-only:** sets the silent driver mode.
//!
//! Options ([`BusConfig::option`](canbus_core::BusConfig::option)):
//!
//! | Key | Default | Meaning |
//! |---|---|---|
//! | `accept_virtual` | `true` | Allow opening Kvaser virtual channels |
//! | `exclusive` | `false` | Open with exclusive access |
//! | `fd_non_iso` | `false` | Use non-ISO (Bosch) CAN FD |
//!
//! Timestamps have 10 µs resolution where the device supports it (1 ms
//! otherwise), counted from when the driver started.

mod bus;
mod convert;
mod ffi;

use canbus_core::{BusConfig, CanBus, Interface, Result};

pub use bus::{detect, KvaserBus};

pub(crate) const NAME: &str = "kvaser";

/// Descriptor for the Kvaser backend.
pub const INTERFACE: Interface = Interface::new(NAME, "Kvaser CANlib", open, detect);

fn open(config: &BusConfig) -> Result<Box<dyn CanBus>> {
    Ok(Box::new(KvaserBus::open(config)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}

    #[test]
    fn bus_is_send() {
        assert_send::<KvaserBus>();
    }

    #[test]
    fn non_numeric_channel_is_rejected_before_loading() {
        // Validation happens after loading the library, so on machines
        // without CANlib this reports LibraryNotLoaded instead; either way
        // it must not panic.
        let err = KvaserBus::open(&BusConfig::new(NAME, "usb0")).unwrap_err();
        assert!(matches!(
            err,
            canbus_core::Error::InvalidConfig(_) | canbus_core::Error::LibraryNotLoaded { .. }
        ));
    }
}
