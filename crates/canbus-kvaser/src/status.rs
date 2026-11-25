//! Kvaser-specific conversions for status/statistics types from `canbus-core`.

use crate::ffi as sys;
use canbus_core::{BusStatistics, BusStatus};

/// Convert a Kvaser `canSTAT_*` bitmap into a [`BusStatus`].
pub(crate) fn bus_status_from_canlib(bits: std::os::raw::c_ulong) -> BusStatus {
    let mut out = BusStatus::empty();
    if bits & sys::canSTAT_ERROR_PASSIVE != 0 {
        out |= BusStatus::ERROR_PASSIVE;
    }
    if bits & sys::canSTAT_BUS_OFF != 0 {
        out |= BusStatus::BUS_OFF;
    }
    if bits & sys::canSTAT_ERROR_WARNING != 0 {
        out |= BusStatus::ERROR_WARNING;
    }
    if bits & sys::canSTAT_ERROR_ACTIVE != 0 {
        out |= BusStatus::ERROR_ACTIVE;
    }
    if bits & sys::canSTAT_TX_PENDING != 0 {
        out |= BusStatus::TX_PENDING;
    }
    if bits & sys::canSTAT_RX_PENDING != 0 {
        out |= BusStatus::RX_PENDING;
    }
    if bits & sys::canSTAT_OVERRUN != 0 {
        out |= BusStatus::OVERRUN;
    }
    out
}

/// Build a [`BusStatistics`] from the raw CANLib structure.
///
/// The raw fields are `c_ulong`: 32-bit on Windows, 64-bit elsewhere, so
/// the widening `u64::from` is a no-op on some targets.
#[allow(clippy::useless_conversion)]
pub(crate) fn bus_statistics_from_raw(raw: &sys::canBusStatistics) -> BusStatistics {
    BusStatistics {
        std_data: u64::from(raw.stdData),
        std_remote: u64::from(raw.stdRemote),
        ext_data: u64::from(raw.extData),
        ext_remote: u64::from(raw.extRemote),
        err_frames: u64::from(raw.errFrame),
        bus_load: u64::from(raw.busLoad),
        overruns: u64::from(raw.overruns),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_status_from_canlib_translates_all_bits() {
        let bits = sys::canSTAT_ERROR_ACTIVE | sys::canSTAT_TX_PENDING;
        let s = bus_status_from_canlib(bits);
        assert!(s.contains(BusStatus::ERROR_ACTIVE));
        assert!(s.contains(BusStatus::TX_PENDING));
        assert!(!s.contains(BusStatus::BUS_OFF));
    }

    #[test]
    fn bus_statistics_round_trip() {
        let raw = sys::canBusStatistics {
            stdData: 10,
            stdRemote: 20,
            extData: 30,
            extRemote: 40,
            errFrame: 5,
            busLoad: 1234,
            overruns: 2,
        };
        let s = bus_statistics_from_raw(&raw);
        assert_eq!(s.std_data, 10);
        assert_eq!(s.bus_load, 1234);
    }
}
