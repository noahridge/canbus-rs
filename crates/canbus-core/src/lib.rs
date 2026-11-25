//! Vendor-agnostic CAN traits, message types, and embedded-can integration.
//!
//! This crate defines the shared surface that backend crates (`canbus-kvaser`,
//! `canbus-pcan`) implement. Generic code can be written against [`CanChannel`]
//! (or one of the sub-traits) and stay portable across backends.

pub mod bus_params;
pub mod channel;
pub mod embedded_can_glue;
pub mod error;
pub mod message;
pub mod status;

#[cfg(feature = "mock")]
pub mod mock;

// Re-export embedded-can so downstream crates can access trait/id types
// without a separate dependency declaration.
pub use embedded_can;

// Used by the exported `impl_embedded_can!` macro so callers don't need
// their own `nb` dependency.
#[doc(hidden)]
pub use nb;

pub use bus_params::{Bitrate, BusParams, BusParamsTq, DriverType, FdBitrate};
pub use channel::{CanBusControl, CanChannel, CanDiagnostics, CanRead, CanWrite, Filter};
pub use embedded_can::ErrorKind as BusErrorKind;
pub use embedded_can::{ExtendedId, Frame, Id, StandardId};
pub use embedded_can_glue::{
    blocking_receive, blocking_transmit, decode_error_frame, nb_receive, nb_transmit, BusError,
    EmbeddedCanError,
};
pub use error::{CanError, Result};
pub use message::{
    id_to_raw_and_flag, Brs, CanMessage, DataFrame, ErrorFrame, MessageFlags, RemoteFrame,
    CANFD_MAX_DLC, CAN_EXT_ID_MAX, CAN_MAX_DLC, CAN_STD_ID_MAX,
};
pub use status::{BusStatistics, BusStatus, ErrorCounters};
