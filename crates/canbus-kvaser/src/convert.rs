//! Translation between `canbus-core` types and CANlib's native values.

use std::os::raw::{c_char, c_long, c_uint, c_ulong};
use std::time::Duration;

use canbus_core::{BusState, Error, MessageFlags, Result};

use crate::ffi as sys;
use crate::NAME;

/// Get the loaded CANlib, or [`Error::LibraryNotLoaded`].
pub(crate) fn lib() -> Result<&'static sys::CanLib> {
    sys::get().map_err(|e| Error::LibraryNotLoaded {
        interface: NAME,
        reason: e.to_string(),
    })
}

/// Turn a negative `canStatus` into an [`Error`]; non-negative is success.
pub(crate) fn check(lib: &sys::CanLib, status: sys::canStatus) -> Result<()> {
    if status >= sys::canOK {
        Ok(())
    } else {
        Err(error_from_status(lib, status))
    }
}

/// Map a CANlib status code onto the vendor-neutral [`Error`].
pub(crate) fn error_from_status(lib: &sys::CanLib, status: sys::canStatus) -> Error {
    match status {
        sys::canERR_TIMEOUT => Error::Timeout,
        sys::canERR_TXBUFOFL => Error::TxQueueFull,
        sys::canERR_NOTFOUND | sys::canERR_NOCHANNELS | sys::canERR_NOCARD => {
            Error::ChannelNotFound
        }
        sys::canERR_NO_ACCESS => Error::ChannelInUse,
        sys::canERR_NOT_SUPPORTED | sys::canERR_NOT_IMPLEMENTED => {
            Error::Unsupported("not supported by this Kvaser device or driver")
        }
        sys::canERR_HARDWARE => Error::Hardware,
        sys::canERR_DRIVER | sys::canERR_DRIVERLOAD | sys::canERR_DRIVERFAILED => Error::Driver,
        other => Error::Backend {
            interface: NAME,
            code: other.into(),
            message: error_text(lib, other),
        },
    }
}

/// CANlib's description of a status code.
fn error_text(lib: &sys::CanLib, status: sys::canStatus) -> String {
    let mut buf = [0u8; 256];
    // SAFETY: CANlib writes at most `buf.len()` bytes, NUL-terminated.
    let rc = unsafe {
        (lib.canGetErrorText)(status, buf.as_mut_ptr() as *mut c_char, buf.len() as c_uint)
    };
    if rc < 0 {
        return format!("unknown CANlib status {status}");
    }
    c_buf_to_string(&buf)
}

/// Read a NUL-terminated byte buffer as (lossy) UTF-8.
pub(crate) fn c_buf_to_string(buf: &[u8]) -> String {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

/// Translate a [`MessageFlags`] value into the raw CANlib flag bits.
pub(crate) fn flags_to_canlib(flags: MessageFlags) -> c_uint {
    let mut out = 0;
    for (ours, theirs) in FLAG_MAP {
        if flags.contains(ours) {
            out |= theirs;
        }
    }
    out
}

/// Translate raw CANlib flag bits into a [`MessageFlags`] value.
pub(crate) fn flags_from_canlib(raw: c_uint) -> MessageFlags {
    let mut out = MessageFlags::empty();
    for (ours, theirs) in FLAG_MAP {
        if raw & theirs != 0 {
            out |= ours;
        }
    }
    if raw & sys::canMSG_TXACK != 0 {
        out |= MessageFlags::ECHO;
    }
    out
}

const FLAG_MAP: [(MessageFlags, c_uint); 14] = [
    (MessageFlags::RTR, sys::canMSG_RTR),
    (MessageFlags::STD, sys::canMSG_STD),
    (MessageFlags::EXT, sys::canMSG_EXT),
    (MessageFlags::ERROR_FRAME, sys::canMSG_ERROR_FRAME),
    (MessageFlags::FD, sys::canFDMSG_FDF),
    (MessageFlags::BRS, sys::canFDMSG_BRS),
    (MessageFlags::ESI, sys::canFDMSG_ESI),
    (MessageFlags::ERR_HW_OVERRUN, sys::canMSGERR_HW_OVERRUN),
    (MessageFlags::ERR_SW_OVERRUN, sys::canMSGERR_SW_OVERRUN),
    (MessageFlags::ERR_STUFF, sys::canMSGERR_STUFF),
    (MessageFlags::ERR_FORM, sys::canMSGERR_FORM),
    (MessageFlags::ERR_CRC, sys::canMSGERR_CRC),
    (MessageFlags::ERR_BIT0, sys::canMSGERR_BIT0),
    (MessageFlags::ERR_BIT1, sys::canMSGERR_BIT1),
];

/// Map a `canSTAT_*` bitmap to a [`BusState`], most severe first.
pub(crate) fn bus_state_from_canlib(bits: c_ulong) -> BusState {
    if bits & sys::canSTAT_BUS_OFF != 0 {
        BusState::BusOff
    } else if bits & sys::canSTAT_ERROR_PASSIVE != 0 {
        BusState::Passive
    } else if bits & sys::canSTAT_ERROR_WARNING != 0 {
        BusState::Warning
    } else if bits & sys::canSTAT_ERROR_ACTIVE != 0 {
        BusState::Active
    } else {
        BusState::Unknown
    }
}

/// CANlib's predefined constant for a classic (nominal) bitrate, if any.
pub(crate) fn classic_preset(bitrate: u32) -> Option<c_long> {
    Some(match bitrate {
        1_000_000 => sys::canBITRATE_1M,
        500_000 => sys::canBITRATE_500K,
        250_000 => sys::canBITRATE_250K,
        125_000 => sys::canBITRATE_125K,
        100_000 => sys::canBITRATE_100K,
        83_000 | 83_333 => sys::canBITRATE_83K,
        62_000 | 62_500 => sys::canBITRATE_62K,
        50_000 => sys::canBITRATE_50K,
        10_000 => sys::canBITRATE_10K,
        _ => return None,
    })
}

/// CANlib's predefined constant for a CAN FD data bitrate (80 % sample
/// point), if any.
pub(crate) fn fd_data_preset(bitrate: u32) -> Option<c_long> {
    Some(match bitrate {
        500_000 => sys::canFD_BITRATE_500K_80P,
        1_000_000 => sys::canFD_BITRATE_1M_80P,
        2_000_000 => sys::canFD_BITRATE_2M_80P,
        4_000_000 => sys::canFD_BITRATE_4M_80P,
        8_000_000 => sys::canFD_BITRATE_8M_80P,
        _ => return None,
    })
}

/// CANlib's "wait forever" timeout value.
const INFINITE: c_ulong = 0xFFFF_FFFF;

/// Convert a [`canbus_core::CanBus`] timeout to CANlib milliseconds,
/// rounding up so a sub-millisecond wait doesn't become non-blocking.
pub(crate) fn timeout_ms(timeout: Option<Duration>) -> c_ulong {
    match timeout {
        None => INFINITE,
        Some(t) => {
            let ms = t.as_micros().div_ceil(1000);
            ms.min(u128::from(INFINITE - 1)) as c_ulong
        }
    }
}

/// Translate an inclusive ID range into a code/mask pair.
///
/// A range is exactly representable as code/mask only when its size is a
/// power of two and `from` is aligned to that size (e.g. `0x100..=0x103`).
/// Returns `None` for anything else, including invalid ranges.
pub(crate) fn range_to_code_mask(from: u32, to: u32, extended: bool) -> Option<(u32, u32)> {
    let id_bits: u32 = if extended { 0x1FFF_FFFF } else { 0x7FF };
    if from > to || to > id_bits {
        return None;
    }
    let size = to - from + 1;
    if !size.is_power_of_two() || from & (size - 1) != 0 {
        return None;
    }
    let mask = !(size - 1) & id_bits;
    Some((from & mask, mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_round_trip_through_canlib() {
        let cases = [
            MessageFlags::STD,
            MessageFlags::EXT,
            MessageFlags::STD | MessageFlags::RTR,
            MessageFlags::STD | MessageFlags::FD | MessageFlags::BRS,
            MessageFlags::ERROR_FRAME | MessageFlags::ERR_BIT0,
            MessageFlags::EXT | MessageFlags::FD | MessageFlags::ESI,
        ];
        for f in cases {
            assert_eq!(flags_from_canlib(flags_to_canlib(f)), f);
        }
        assert!(flags_from_canlib(sys::canMSG_TXACK).contains(MessageFlags::ECHO));
    }

    #[test]
    fn bus_state_picks_most_severe() {
        let all = sys::canSTAT_ERROR_ACTIVE | sys::canSTAT_ERROR_WARNING | sys::canSTAT_BUS_OFF;
        assert_eq!(bus_state_from_canlib(all), BusState::BusOff);
        assert_eq!(
            bus_state_from_canlib(sys::canSTAT_ERROR_PASSIVE),
            BusState::Passive
        );
        assert_eq!(
            bus_state_from_canlib(sys::canSTAT_ERROR_ACTIVE | sys::canSTAT_TX_PENDING),
            BusState::Active
        );
        assert_eq!(bus_state_from_canlib(0), BusState::Unknown);
    }

    #[test]
    fn presets() {
        assert_eq!(classic_preset(500_000), Some(sys::canBITRATE_500K));
        assert_eq!(classic_preset(83_333), Some(sys::canBITRATE_83K));
        assert_eq!(classic_preset(123_456), None);
        assert_eq!(fd_data_preset(2_000_000), Some(sys::canFD_BITRATE_2M_80P));
        assert_eq!(fd_data_preset(3_000_000), None);
    }

    #[test]
    fn timeouts() {
        assert_eq!(timeout_ms(None), INFINITE);
        assert_eq!(timeout_ms(Some(Duration::ZERO)), 0);
        assert_eq!(timeout_ms(Some(Duration::from_micros(1))), 1);
        assert_eq!(timeout_ms(Some(Duration::from_millis(250))), 250);
        assert_eq!(timeout_ms(Some(Duration::MAX)), INFINITE - 1);
    }

    #[test]
    fn aligned_range_maps_to_code_mask() {
        assert_eq!(
            range_to_code_mask(0x100, 0x103, false),
            Some((0x100, 0x7FC))
        );
        assert_eq!(
            range_to_code_mask(0x123, 0x123, false),
            Some((0x123, 0x7FF))
        );
        assert_eq!(range_to_code_mask(0, 0x7FF, false), Some((0, 0)));
        assert_eq!(range_to_code_mask(0, 0x1FFF_FFFF, true), Some((0, 0)));
    }

    #[test]
    fn unaligned_odd_sized_or_invalid_range_is_rejected() {
        // Previously mistranslated: 1..=3 produced a filter that also accepted 0.
        assert_eq!(range_to_code_mask(1, 3, false), None);
        assert_eq!(range_to_code_mask(7, 15, false), None);
        assert_eq!(range_to_code_mask(0, 2, false), None);
        assert_eq!(range_to_code_mask(5, 4, false), None);
        assert_eq!(range_to_code_mask(0, 0x800, false), None);
    }

    #[test]
    fn accepted_ranges_match_exactly() {
        // Exhaustive over small standard IDs: whenever a range is accepted,
        // the code/mask filter must pass exactly the IDs in the range.
        for from in 0..0x40u32 {
            for to in from..0x40 {
                if let Some((code, mask)) = range_to_code_mask(from, to, false) {
                    for id in 0..=0x7FFu32 {
                        assert_eq!(
                            id & mask == code,
                            (from..=to).contains(&id),
                            "range {from:#x}..={to:#x}, id {id:#x}"
                        );
                    }
                }
            }
        }
    }
}
