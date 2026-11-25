//! Vendor-agnostic CAN error type.
//!
//! Backends translate their native status codes into [`CanError`]. The
//! variant set spans the union of conditions exposed by the supported
//! backends; backends signal errors they don't natively distinguish via
//! the closest available variant (or [`CanError::Backend`] as a catch-all).

/// Result type alias used throughout the canbus crates.
pub type Result<T> = core::result::Result<T, CanError>;

/// Error type representing CAN driver and bus conditions across backends.
///
/// The `Backend` variant is the catch-all for vendor-specific codes that
/// don't map cleanly onto the unified set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CanError {
    /// The vendor shared library could not be loaded.
    #[error("CAN library not loaded: {0}")]
    LibraryNotLoaded(String),
    /// Invalid parameter passed to a CAN call.
    #[error("invalid parameter")]
    Param,
    /// No message available in the receive queue.
    #[error("no message available")]
    NoMsg,
    /// A receive buffer overrun (HW or SW).
    #[error("receive overrun")]
    Overrun,
    /// Channel or device not found.
    #[error("channel not found")]
    NotFound,
    /// Out of memory.
    #[error("out of memory")]
    NoMem,
    /// No CAN channels available.
    #[error("no channels available")]
    NoChannels,
    /// Operation interrupted.
    #[error("operation interrupted")]
    Interrupted,
    /// Operation timed out.
    #[error("operation timed out")]
    Timeout,
    /// Library not initialized.
    #[error("library not initialized")]
    NotInitialized,
    /// No free handles.
    #[error("no free handles")]
    NoHandles,
    /// Invalid channel handle.
    #[error("invalid handle")]
    InvalidHandle,
    /// Ini-file error (Kvaser).
    #[error("ini-file error")]
    IniFile,
    /// Generic driver error.
    #[error("driver error")]
    Driver,
    /// Transmit buffer overflow.
    #[error("transmit buffer overflow")]
    TxBufOverflow,
    /// Hardware error.
    #[error("hardware error")]
    Hardware,
    /// Bus is off.
    #[error("bus off")]
    BusOff,
    /// Dynamic library load error.
    #[error("dynamic library load error")]
    DynaLoad,
    /// Dynamic library not found.
    #[error("dynamic library not found")]
    DynaLib,
    /// Dynamic library init error.
    #[error("dynamic library init error")]
    DynaInit,
    /// Operation not supported by this hardware/driver.
    #[error("operation not supported")]
    NotSupported,
    /// Driver load error.
    #[error("driver load error")]
    DriverLoad,
    /// Driver failed.
    #[error("driver failed")]
    DriverFailed,
    /// No configuration manager.
    #[error("no configuration manager")]
    NoConfigMgr,
    /// No card found.
    #[error("no card found")]
    NoCard,
    /// Registry error.
    #[error("registry error")]
    Registry,
    /// License error.
    #[error("license error")]
    License,
    /// Internal driver error.
    #[error("internal error")]
    Internal,
    /// Access denied.
    #[error("no access")]
    NoAccess,
    /// Feature not implemented for this backend.
    #[error("not implemented")]
    NotImplemented,
    /// Device-file error.
    #[error("device file error")]
    DeviceFile,
    /// Host-file error.
    #[error("host file error")]
    HostFile,
    /// Disk error.
    #[error("disk error")]
    Disk,
    /// CRC mismatch reported by the driver.
    #[error("CRC error")]
    Crc,
    /// Driver configuration error.
    #[error("configuration error")]
    Config,
    /// Memo (data-logger) failure.
    #[error("memo failure")]
    MemoFail,
    /// Script failure.
    #[error("script failure")]
    ScriptFail,
    /// Script version mismatch.
    #[error("script wrong version")]
    ScriptWrongVersion,
    /// Catch-all for vendor codes that don't map onto a specific variant.
    /// The integer is the backend's raw status value, sign-extended if needed.
    #[error("backend error (code {0})")]
    Backend(i32),
}
