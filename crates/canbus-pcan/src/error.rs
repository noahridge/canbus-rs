//! PCAN-Basic status code translation onto [`canbus_core::CanError`].

use crate::ffi as sys;
use canbus_core::{CanError, Result};

/// Get a reference to the loaded PCAN-Basic library.
pub(crate) fn lib() -> Result<&'static sys::PcanLib> {
    sys::get().map_err(|e| CanError::LibraryNotLoaded(e.to_string()))
}

/// Translate a `TPCANStatus` bitmap into a [`Result`].
///
/// `PCAN_ERROR_OK` (and `PCAN_ERROR_CAUTION` alone) is success. Otherwise
/// the most-specific known error bit wins. Unrecognised bit patterns map
/// to [`CanError::Backend`].
pub fn check_status(status: sys::TPCANStatus) -> Result<()> {
    // Strip "caution" — it's an info bit, not a failure.
    let s = status & !sys::PCAN_ERROR_CAUTION;
    if s == sys::PCAN_ERROR_OK {
        return Ok(());
    }
    Err(from_pcan_status(s))
}

/// Convert a non-zero `TPCANStatus` value into a [`CanError`].
pub fn from_pcan_status(status: sys::TPCANStatus) -> CanError {
    // Order is most-specific first; PCAN sometimes ORs multiple bits.
    if status & sys::PCAN_ERROR_QRCVEMPTY != 0 {
        CanError::NoMsg
    } else if status & sys::PCAN_ERROR_QOVERRUN != 0 || status & sys::PCAN_ERROR_OVERRUN != 0 {
        CanError::Overrun
    } else if status & sys::PCAN_ERROR_QXMTFULL != 0 || status & sys::PCAN_ERROR_XMTFULL != 0 {
        CanError::TxBufOverflow
    } else if status & sys::PCAN_ERROR_BUSOFF != 0 {
        CanError::BusOff
    } else if status & sys::PCAN_ERROR_NODRIVER != 0 {
        CanError::Driver
    } else if status & sys::PCAN_ERROR_REGTEST != 0 {
        CanError::Hardware
    } else if status & (sys::PCAN_ERROR_ILLHW | sys::PCAN_ERROR_ILLNET | sys::PCAN_ERROR_ILLCLIENT)
        != 0
    {
        CanError::InvalidHandle
    } else if status & sys::PCAN_ERROR_HWINUSE != 0 || status & sys::PCAN_ERROR_NETINUSE != 0 {
        CanError::NoAccess
    } else if status & sys::PCAN_ERROR_RESOURCE != 0 {
        CanError::NoMem
    } else if status & (sys::PCAN_ERROR_ILLPARAMTYPE | sys::PCAN_ERROR_ILLPARAMVAL) != 0 {
        CanError::Param
    } else if status & sys::PCAN_ERROR_INITIALIZE != 0 {
        CanError::NotInitialized
    } else if status & sys::PCAN_ERROR_ILLOPERATION != 0 || status & sys::PCAN_ERROR_ILLMODE != 0 {
        CanError::NotSupported
    } else if status & sys::PCAN_ERROR_ILLDATA != 0 {
        CanError::Param
    } else {
        CanError::Backend(status as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_status_passes() {
        assert!(check_status(sys::PCAN_ERROR_OK).is_ok());
    }

    #[test]
    fn caution_alone_is_ok() {
        assert!(check_status(sys::PCAN_ERROR_CAUTION).is_ok());
    }

    #[test]
    fn empty_queue_maps_to_no_msg() {
        assert_eq!(from_pcan_status(sys::PCAN_ERROR_QRCVEMPTY), CanError::NoMsg);
    }

    #[test]
    fn xmt_full_maps_to_txbufoverflow() {
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_QXMTFULL),
            CanError::TxBufOverflow
        );
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_XMTFULL),
            CanError::TxBufOverflow
        );
    }

    #[test]
    fn bus_off_maps() {
        assert_eq!(from_pcan_status(sys::PCAN_ERROR_BUSOFF), CanError::BusOff);
    }

    #[test]
    fn invalid_handle_family() {
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_ILLHW),
            CanError::InvalidHandle
        );
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_ILLNET),
            CanError::InvalidHandle
        );
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_ILLCLIENT),
            CanError::InvalidHandle
        );
    }

    #[test]
    fn param_errors_map() {
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_ILLPARAMVAL),
            CanError::Param
        );
        assert_eq!(
            from_pcan_status(sys::PCAN_ERROR_ILLPARAMTYPE),
            CanError::Param
        );
    }

    #[test]
    fn unknown_falls_back_to_backend() {
        let e = from_pcan_status(0x10000); // PCAN_ERROR_UNKNOWN
        assert!(matches!(e, CanError::Backend(_)));
    }
}
