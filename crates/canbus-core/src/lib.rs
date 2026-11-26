//! The stable contract of the `canbus` crate family.
//!
//! This crate defines what every CAN backend implements and what every
//! application programs against:
//!
//! - [`CanMessage`] — a classic or FD frame
//! - [`BusConfig`] — which interface/channel to open, bitrates and options
//! - [`CanBus`] — an open bus: send, receive, filter, status
//! - [`Interface`] — a backend descriptor; backend crates export one as
//!   `INTERFACE`
//! - [`Error`] — vendor-neutral errors
//!
//! Applications normally depend on the `canbus` facade crate, which
//! re-exports everything here and selects backends at runtime. Depend on
//! `canbus-core` directly only to write a backend.
//!
//! # Writing a backend
//!
//! Implement [`CanBus`] for your bus type and export a descriptor:
//!
//! ```
//! use canbus_core::{BusConfig, CanBus, ChannelInfo, Interface, Result};
//!
//! fn open(config: &BusConfig) -> Result<Box<dyn CanBus>> {
//!     # let _ = config;
//!     # Err(canbus_core::Error::ChannelNotFound)
//!     // Validate `config`, open the device, return Box::new(MyBus { .. })
//! }
//!
//! fn detect() -> Result<Vec<ChannelInfo>> {
//!     Ok(Vec::new())
//! }
//!
//! pub const INTERFACE: Interface = Interface::new("mybackend", "My CAN adapter", open, detect);
//! ```

mod bus;
mod config;
mod embedded;
mod error;
mod filter;
mod frame;
mod interface;

// Re-exported so downstream crates can use the ID and trait types without
// declaring their own dependency.
pub use embedded_can;
pub use nb;

pub use bus::{BusState, BusStatus, CanBus, Capabilities, Deadline, ErrorCounters};
pub use config::{BusConfig, FdConfig, Timing, parse_bool};
pub use embedded::{
    BusError, EmbeddedCan, EmbeddedCanError, blocking_receive, blocking_transmit,
    decode_error_frame, nb_receive, nb_transmit,
};
pub use embedded_can::ErrorKind as BusErrorKind;
pub use embedded_can::{ExtendedId, Id, StandardId};
pub use error::{Error, Result};
pub use filter::{Filter, FilterSupport};
pub use frame::{
    Brs, CAN_EXT_ID_MAX, CAN_MAX_LEN, CAN_STD_ID_MAX, CANFD_MAX_LEN, CanMessage, DataFrame,
    ErrorFrame, MessageFlags, RemoteFrame, dlc_to_len, fd_len_round_up, id_to_raw_and_flag,
    len_to_dlc,
};
pub use interface::{ChannelInfo, DetectFn, Interface, OpenFn};
