//! Kvaser-specific status code translation onto [`canbus_core::CanError`].
//!
//! The error type itself lives in `canbus-core` and is re-exported from the
//! crate root. This module just maps `canStatus` integers to/from
//! `CanError` variants and provides the FFI library accessor used
//! internally.

use crate::ffi as sys;
use canbus_core::{CanError, Result};

/// Convert a `canStatus` code into a [`CanError`].
pub fn from_canlib_status(status: sys::canStatus) -> CanError {
    match status {
        sys::canERR_PARAM => CanError::Param,
        sys::canERR_NOMSG => CanError::NoMsg,
        sys::canERR_NOTFOUND => CanError::NotFound,
        sys::canERR_NOMEM => CanError::NoMem,
        sys::canERR_NOCHANNELS => CanError::NoChannels,
        sys::canERR_INTERRUPTED => CanError::Interrupted,
        sys::canERR_TIMEOUT => CanError::Timeout,
        sys::canERR_NOTINITIALIZED => CanError::NotInitialized,
        sys::canERR_NOHANDLES => CanError::NoHandles,
        sys::canERR_INVHANDLE => CanError::InvalidHandle,
        sys::canERR_INIFILE => CanError::IniFile,
        sys::canERR_DRIVER => CanError::Driver,
        sys::canERR_TXBUFOFL => CanError::TxBufOverflow,
        sys::canERR_HARDWARE => CanError::Hardware,
        sys::canERR_DYNALOAD => CanError::DynaLoad,
        sys::canERR_DYNALIB => CanError::DynaLib,
        sys::canERR_DYNAINIT => CanError::DynaInit,
        sys::canERR_NOT_SUPPORTED => CanError::NotSupported,
        sys::canERR_DRIVERLOAD => CanError::DriverLoad,
        sys::canERR_DRIVERFAILED => CanError::DriverFailed,
        sys::canERR_NOCONFIGMGR => CanError::NoConfigMgr,
        sys::canERR_NOCARD => CanError::NoCard,
        sys::canERR_REGISTRY => CanError::Registry,
        sys::canERR_LICENSE => CanError::License,
        sys::canERR_INTERNAL => CanError::Internal,
        sys::canERR_NO_ACCESS => CanError::NoAccess,
        sys::canERR_NOT_IMPLEMENTED => CanError::NotImplemented,
        sys::canERR_DEVICE_FILE => CanError::DeviceFile,
        sys::canERR_HOST_FILE => CanError::HostFile,
        sys::canERR_DISK => CanError::Disk,
        sys::canERR_CRC => CanError::Crc,
        sys::canERR_CONFIG => CanError::Config,
        sys::canERR_MEMO_FAIL => CanError::MemoFail,
        sys::canERR_SCRIPT_FAIL => CanError::ScriptFail,
        sys::canERR_SCRIPT_WRONG_VERSION => CanError::ScriptWrongVersion,
        other => CanError::Backend(other),
    }
}

/// Convert a [`CanError`] back to the corresponding raw `canStatus` code.
///
/// Used internally by [`crate::get_error_text`]. Variants that don't map
/// onto a Kvaser status return `canERR_NOT_IMPLEMENTED`.
pub fn to_canlib_status(err: &CanError) -> sys::canStatus {
    match err {
        CanError::LibraryNotLoaded(_) => sys::canERR_DYNALIB,
        CanError::Param => sys::canERR_PARAM,
        CanError::NoMsg => sys::canERR_NOMSG,
        CanError::NotFound => sys::canERR_NOTFOUND,
        CanError::NoMem => sys::canERR_NOMEM,
        CanError::NoChannels => sys::canERR_NOCHANNELS,
        CanError::Interrupted => sys::canERR_INTERRUPTED,
        CanError::Timeout => sys::canERR_TIMEOUT,
        CanError::NotInitialized => sys::canERR_NOTINITIALIZED,
        CanError::NoHandles => sys::canERR_NOHANDLES,
        CanError::InvalidHandle => sys::canERR_INVHANDLE,
        CanError::IniFile => sys::canERR_INIFILE,
        CanError::Driver => sys::canERR_DRIVER,
        CanError::TxBufOverflow => sys::canERR_TXBUFOFL,
        CanError::Hardware => sys::canERR_HARDWARE,
        CanError::DynaLoad => sys::canERR_DYNALOAD,
        CanError::DynaLib => sys::canERR_DYNALIB,
        CanError::DynaInit => sys::canERR_DYNAINIT,
        CanError::NotSupported => sys::canERR_NOT_SUPPORTED,
        CanError::DriverLoad => sys::canERR_DRIVERLOAD,
        CanError::DriverFailed => sys::canERR_DRIVERFAILED,
        CanError::NoConfigMgr => sys::canERR_NOCONFIGMGR,
        CanError::NoCard => sys::canERR_NOCARD,
        CanError::Registry => sys::canERR_REGISTRY,
        CanError::License => sys::canERR_LICENSE,
        CanError::Internal => sys::canERR_INTERNAL,
        CanError::NoAccess => sys::canERR_NO_ACCESS,
        CanError::NotImplemented => sys::canERR_NOT_IMPLEMENTED,
        CanError::DeviceFile => sys::canERR_DEVICE_FILE,
        CanError::HostFile => sys::canERR_HOST_FILE,
        CanError::Disk => sys::canERR_DISK,
        CanError::Crc => sys::canERR_CRC,
        CanError::Config => sys::canERR_CONFIG,
        CanError::MemoFail => sys::canERR_MEMO_FAIL,
        CanError::ScriptFail => sys::canERR_SCRIPT_FAIL,
        CanError::ScriptWrongVersion => sys::canERR_SCRIPT_WRONG_VERSION,
        CanError::Backend(code) => *code,
        // Variants without a direct Kvaser counterpart.
        CanError::Overrun | CanError::BusOff => sys::canERR_NOT_IMPLEMENTED,
        _ => sys::canERR_NOT_IMPLEMENTED,
    }
}

/// Check a `canStatus` return value and convert to `Result<()>`.
pub(crate) fn check_status(status: sys::canStatus) -> Result<()> {
    if status >= sys::canOK {
        Ok(())
    } else {
        Err(from_canlib_status(status))
    }
}

/// Check a `canStatus` return value that may be a handle (non-negative = success).
pub(crate) fn check_handle(status: sys::canStatus) -> Result<sys::canHandle> {
    if status >= 0 {
        Ok(status)
    } else {
        Err(from_canlib_status(status))
    }
}

/// Get a reference to the loaded CANLib, or return a `LibraryNotLoaded`.
pub(crate) fn lib() -> Result<&'static sys::CanLib> {
    sys::get().map_err(|e| CanError::LibraryNotLoaded(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_status_maps_known_codes() {
        assert_eq!(from_canlib_status(sys::canERR_TIMEOUT), CanError::Timeout);
        assert_eq!(from_canlib_status(sys::canERR_NOMSG), CanError::NoMsg);
        assert_eq!(from_canlib_status(sys::canERR_HARDWARE), CanError::Hardware);
    }

    #[test]
    fn from_status_falls_back_to_backend() {
        assert_eq!(
            from_canlib_status(sys::canERR_RESERVED_1),
            CanError::Backend(sys::canERR_RESERVED_1)
        );
        assert_eq!(from_canlib_status(-999), CanError::Backend(-999));
    }

    #[test]
    fn check_status_ok_for_non_negative() {
        assert!(check_status(0).is_ok());
        assert!(check_status(1).is_ok());
        assert!(check_status(-1).is_err());
    }
}
