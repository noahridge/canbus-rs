//! Cross-platform CAN bus access for Rust, with runtime-selectable backends.
//!
//! `canbus` gives one API for CAN adapters from different vendors, in the
//! spirit of Python's `python-can`: pick the interface and channel at runtime
//! (from code, the command line or environment variables) and the same code
//! talks to Kvaser, PEAK PCAN or an in-process virtual bus.
//!
//! ```
//! use std::time::Duration;
//! use canbus::{Bus, BusConfig, CanBus, CanMessage, StandardId};
//!
//! # fn main() -> canbus::Result<()> {
//! // Two endpoints on the same virtual channel.
//! let config = BusConfig::new("virtual", "doc-example").bitrate(500_000);
//! let mut a = Bus::open(&config)?;
//! let mut b = Bus::open(&config)?;
//!
//! let id = StandardId::new(0x123).unwrap();
//! a.send(&CanMessage::new(id, &[0xDE, 0xAD, 0xBE, 0xEF])?, None)?;
//!
//! let rx = b.recv(Some(Duration::from_secs(1)))?.expect("frame");
//! assert_eq!(rx.data(), &[0xDE, 0xAD, 0xBE, 0xEF]);
//! # Ok(())
//! # }
//! ```
//!
//! # Interfaces
//!
//! | Name | Feature | Channel format | Needs |
//! |---|---|---|---|
//! | `kvaser` | `kvaser` | channel index, e.g. `"0"` | Kvaser CANlib (Windows/Linux) |
//! | `pcan` | `pcan` | `"PCAN_USBBUS1"`, `"PCAN_PCIBUS2"`, … | PEAK PCAN-Basic (Windows/Linux) |
//! | `virtual` | `virtual` | any name, e.g. `"vcan0"` | nothing |
//!
//! All three are enabled by default. Vendor libraries are loaded when a bus
//! is first opened, so enabling a backend never adds a build-time or
//! link-time requirement.
//!
//! Backends outside this crate implement [`CanBus`] and export an
//! [`Interface`]; open them with [`Bus::open_with`].
//!
//! # Environment variables
//!
//! [`Bus::from_env`] reads the same variables as python-can:
//! `CAN_INTERFACE`, `CAN_CHANNEL`, `CAN_BITRATE`, `CAN_DATA_BITRATE`,
//! `CAN_FD` and `CAN_LISTEN_ONLY`. See [`config_from_env`].

mod bus;
mod env;

pub use bus::Bus;
pub use canbus_core::*;
pub use env::{config_from_env, config_from_lookup};

/// The Kvaser CANlib backend.
#[cfg(feature = "kvaser")]
pub use canbus_kvaser as kvaser;

/// The PEAK PCAN-Basic backend.
#[cfg(feature = "pcan")]
pub use canbus_pcan as pcan;

/// The in-process virtual backend.
#[cfg(feature = "virtual")]
pub use canbus_virtual as virtual_bus;

const INTERFACES: &[Interface] = &[
    #[cfg(feature = "kvaser")]
    canbus_kvaser::INTERFACE,
    #[cfg(feature = "pcan")]
    canbus_pcan::INTERFACE,
    #[cfg(feature = "virtual")]
    canbus_virtual::INTERFACE,
];

/// The interfaces compiled into this build.
pub fn interfaces() -> &'static [Interface] {
    INTERFACES
}

/// Look up a compiled-in interface by name (case-insensitive).
pub fn interface(name: &str) -> Option<&'static Interface> {
    INTERFACES
        .iter()
        .find(|i| i.name.eq_ignore_ascii_case(name))
}

/// List the channels every compiled-in interface can currently see.
///
/// Interfaces whose vendor library isn't installed, or that fail to
/// enumerate, are skipped.
pub fn detect_available() -> Vec<ChannelInfo> {
    INTERFACES
        .iter()
        .filter_map(|i| i.detect().ok())
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_lookup_is_case_insensitive() {
        #[cfg(feature = "virtual")]
        assert_eq!(interface("Virtual").map(|i| i.name), Some("virtual"));
        assert!(interface("no-such-backend").is_none());
    }

    #[test]
    fn all_default_interfaces_are_listed() {
        let names: Vec<_> = interfaces().iter().map(|i| i.name).collect();
        #[cfg(feature = "kvaser")]
        assert!(names.contains(&"kvaser"));
        #[cfg(feature = "pcan")]
        assert!(names.contains(&"pcan"));
        #[cfg(feature = "virtual")]
        assert!(names.contains(&"virtual"));
        let _ = names;
    }
}
