//! Kvaser-specific conversions for status/statistics types from `can-core`.

use canlib_sys as sys;
use can_core::{BusStatistics, BusStatus};

/// Convert a Kvaser `canSTAT_*` bitmap into a [`BusStatus`].
pub(crate) fn bus_status_from_canlib(bits: u64) -> BusStatus {
    let mut out = BusStatus::empty();
    if bits & (sys::canSTAT_ERROR_PASSIVE as u64) != 0 { out |= BusStatus::ERROR_PASSIVE; }
    if bits & (sys::canSTAT_BUS_OFF as u64) != 0 { out |= BusStatus::BUS_OFF; }
    if bits & (sys::canSTAT_ERROR_WARNING as u64) != 0 { out |= BusStatus::ERROR_WARNING; }
    if bits & (sys::canSTAT_ERROR_ACTIVE as u64) != 0 { out |= BusStatus::ERROR_ACTIVE; }
    if bits & (sys::canSTAT_TX_PENDING as u64) != 0 { out |= BusStatus::TX_PENDING; }
    if bits & (sys::canSTAT_RX_PENDING as u64) != 0 { out |= BusStatus::RX_PENDING; }
    if bits & (sys::canSTAT_OVERRUN as u64) != 0 { out |= BusStatus::OVERRUN; }
    out
}

/// Build a [`BusStatistics`] from the raw CANLib structure.
pub(crate) fn bus_statistics_from_raw(raw: &sys::canBusStatistics) -> BusStatistics {
    BusStatistics {
        std_data: raw.stdData as u64,
        std_remote: raw.stdRemote as u64,
        ext_data: raw.extData as u64,
        ext_remote: raw.extRemote as u64,
        err_frames: raw.errFrame as u64,
        bus_load: raw.busLoad as u64,
        overruns: raw.overruns as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_status_from_canlib_translates_all_bits() {
        let bits = (sys::canSTAT_ERROR_ACTIVE | sys::canSTAT_TX_PENDING) as u64;
        let s = bus_status_from_canlib(bits);
        assert!(s.contains(BusStatus::ERROR_ACTIVE));
        assert!(s.contains(BusStatus::TX_PENDING));
        assert!(!s.contains(BusStatus::BUS_OFF));
    }

    #[test]
    fn bus_statistics_round_trip() {
        let raw = sys::canBusStatistics {
            stdData: 10, stdRemote: 20, extData: 30, extRemote: 40,
            errFrame: 5, busLoad: 1234, overruns: 2,
        };
        let s = bus_statistics_from_raw(&raw);
        assert_eq!(s.std_data, 10);
        assert_eq!(s.bus_load, 1234);
    }
}
