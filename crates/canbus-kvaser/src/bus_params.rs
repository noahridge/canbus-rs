//! Kvaser-specific translation of `canbus-core` bus-parameter types into the
//! raw values expected by `canSetBusParams` / `canSetBusOutputControl`.
//!
//! The public types ([`Bitrate`], [`FdBitrate`], [`BusParams`](crate::BusParams),
//! [`BusParamsTq`], [`DriverType`]) live in `canbus-core` and are re-exported
//! from the crate root. This module just contains the FFI conversions.

use crate::ffi as sys;
use canbus_core::{Bitrate, BusParamsTq, DriverType, FdBitrate};

/// Convert a [`Bitrate`] to the raw CANLib constant.
pub(crate) fn bitrate_to_raw(b: Bitrate) -> std::os::raw::c_long {
    match b {
        Bitrate::Rate1M => sys::canBITRATE_1M,
        Bitrate::Rate500K => sys::canBITRATE_500K,
        Bitrate::Rate250K => sys::canBITRATE_250K,
        Bitrate::Rate125K => sys::canBITRATE_125K,
        Bitrate::Rate100K => sys::canBITRATE_100K,
        Bitrate::Rate83K => sys::canBITRATE_83K,
        Bitrate::Rate62K => sys::canBITRATE_62K,
        Bitrate::Rate50K => sys::canBITRATE_50K,
        Bitrate::Rate10K => sys::canBITRATE_10K,
    }
}

/// Convert an [`FdBitrate`] to the raw CANLib constant.
pub(crate) fn fd_bitrate_to_raw(b: FdBitrate) -> std::os::raw::c_long {
    match b {
        FdBitrate::Rate500K80P => sys::canFD_BITRATE_500K_80P,
        FdBitrate::Rate1M80P => sys::canFD_BITRATE_1M_80P,
        FdBitrate::Rate2M80P => sys::canFD_BITRATE_2M_80P,
        FdBitrate::Rate4M80P => sys::canFD_BITRATE_4M_80P,
        FdBitrate::Rate8M60P => sys::canFD_BITRATE_8M_60P,
        FdBitrate::Rate8M70P => sys::canFD_BITRATE_8M_70P,
        FdBitrate::Rate8M80P => sys::canFD_BITRATE_8M_80P,
    }
}

/// Convert a [`DriverType`] to the raw CANLib constant.
pub(crate) fn driver_type_to_raw(d: DriverType) -> u32 {
    match d {
        DriverType::Normal => sys::canDRIVER_NORMAL,
        DriverType::Silent => sys::canDRIVER_SILENT,
        DriverType::SelfReception => sys::canDRIVER_SELFRECEPTION,
        DriverType::Off => sys::canDRIVER_OFF,
    }
}

/// Convert a raw CANLib driver constant back into a [`DriverType`].
pub(crate) fn driver_type_from_raw(raw: u32) -> Option<DriverType> {
    match raw {
        sys::canDRIVER_NORMAL => Some(DriverType::Normal),
        sys::canDRIVER_SILENT => Some(DriverType::Silent),
        sys::canDRIVER_SELFRECEPTION => Some(DriverType::SelfReception),
        sys::canDRIVER_OFF => Some(DriverType::Off),
        _ => None,
    }
}

/// Convert a [`BusParamsTq`] to the raw `kvBusParamsTq` struct.
pub(crate) fn bus_params_tq_to_raw(p: BusParamsTq) -> sys::kvBusParamsTq {
    sys::kvBusParamsTq {
        tq: p.tq(),
        phase1: p.phase1(),
        phase2: p.phase2(),
        sjw: p.sjw(),
        prop: p.prop(),
        prescaler: p.prescaler(),
    }
}

/// Convert a raw `kvBusParamsTq` to a [`BusParamsTq`].
pub(crate) fn bus_params_tq_from_raw(raw: sys::kvBusParamsTq) -> BusParamsTq {
    BusParamsTq::new(
        raw.tq,
        raw.phase1,
        raw.phase2,
        raw.sjw,
        raw.prop,
        raw.prescaler,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_to_raw_maps_correctly() {
        assert_eq!(bitrate_to_raw(Bitrate::Rate1M), sys::canBITRATE_1M);
        assert_eq!(bitrate_to_raw(Bitrate::Rate500K), sys::canBITRATE_500K);
        assert_eq!(bitrate_to_raw(Bitrate::Rate10K), sys::canBITRATE_10K);
    }

    #[test]
    fn fd_bitrate_to_raw_maps_correctly() {
        assert_eq!(
            fd_bitrate_to_raw(FdBitrate::Rate500K80P),
            sys::canFD_BITRATE_500K_80P
        );
        assert_eq!(
            fd_bitrate_to_raw(FdBitrate::Rate8M80P),
            sys::canFD_BITRATE_8M_80P
        );
    }

    #[test]
    fn driver_type_round_trips() {
        for dt in [
            DriverType::Normal,
            DriverType::Silent,
            DriverType::SelfReception,
            DriverType::Off,
        ] {
            assert_eq!(driver_type_from_raw(driver_type_to_raw(dt)).unwrap(), dt);
        }
        assert!(driver_type_from_raw(99).is_none());
    }

    #[test]
    fn bus_params_tq_round_trips() {
        let p = BusParamsTq::new(10, 5, 3, 1, 2, 4);
        let raw = bus_params_tq_to_raw(p);
        let back = bus_params_tq_from_raw(raw);
        assert_eq!(back.tq(), 10);
        assert_eq!(back.phase1(), 5);
        assert_eq!(back.prescaler(), 4);
    }
}
