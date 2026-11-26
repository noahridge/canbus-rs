//! Vendor-neutral error type.
//!
//! Backends map their native status codes onto [`Error`]. Conditions that
//! don't have a dedicated variant are reported as [`Error::Backend`], which
//! keeps the vendor's code and message for diagnostics.

/// Result type alias used throughout the canbus crates.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors reported by a CAN bus or backend.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The backend's vendor library could not be loaded.
    #[error("{interface}: vendor library could not be loaded: {reason}")]
    LibraryNotLoaded {
        /// Interface name, e.g. `"kvaser"`.
        interface: &'static str,
        /// Loader error message.
        reason: String,
    },
    /// No interface with this name is available (not compiled in or misspelled).
    #[error("unknown interface `{0}`")]
    UnknownInterface(String),
    /// The requested channel does not exist.
    #[error("channel not found")]
    ChannelNotFound,
    /// The channel exists but is in use by another application.
    #[error("channel is in use")]
    ChannelInUse,
    /// The bus configuration is invalid or not supported by this backend.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    /// The frame cannot be built or sent as given.
    #[error("invalid frame: {0}")]
    InvalidFrame(&'static str),
    /// The operation is not supported by this backend or device.
    #[error("not supported: {0}")]
    Unsupported(&'static str),
    /// The operation did not complete within the timeout.
    #[error("operation timed out")]
    Timeout,
    /// The transmit queue is full.
    #[error("transmit queue full")]
    TxQueueFull,
    /// Received frames were lost because a buffer overran.
    #[error("receive overrun")]
    Overrun,
    /// The controller is bus-off.
    #[error("bus off")]
    BusOff,
    /// The hardware reported a fault.
    #[error("hardware error")]
    Hardware,
    /// The driver reported a fault.
    #[error("driver error")]
    Driver,
    /// The bus has been shut down.
    #[error("bus has been shut down")]
    Shutdown,
    /// A backend-specific error without a dedicated variant.
    #[error("{interface} error {code}: {message}")]
    Backend {
        /// Interface name, e.g. `"pcan"`.
        interface: &'static str,
        /// The backend's raw status code.
        code: i64,
        /// The backend's description of the error.
        message: String,
    },
}
