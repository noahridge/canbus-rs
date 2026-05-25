//! Bitrate translation for PCAN-Basic.

use can_core::{Bitrate, CanError, FdBitrate};
use pcan_sys as sys;

/// Convert a [`Bitrate`] to the PCAN BTR0/BTR1 code.
///
/// All standard variants map to PCAN-Basic constants; `Rate62K` has no
/// direct PCAN equivalent and returns `Err(CanError::NotSupported)` —
/// callers must use a custom timing string in that case.
pub(crate) fn bitrate_to_btr(b: Bitrate) -> Result<sys::TPCANBaudrate, CanError> {
    Ok(match b {
        Bitrate::Rate1M => sys::PCAN_BAUD_1M,
        Bitrate::Rate500K => sys::PCAN_BAUD_500K,
        Bitrate::Rate250K => sys::PCAN_BAUD_250K,
        Bitrate::Rate125K => sys::PCAN_BAUD_125K,
        Bitrate::Rate100K => sys::PCAN_BAUD_100K,
        Bitrate::Rate83K => sys::PCAN_BAUD_83K,
        Bitrate::Rate50K => sys::PCAN_BAUD_50K,
        Bitrate::Rate10K => sys::PCAN_BAUD_10K,
        Bitrate::Rate62K => return Err(CanError::NotSupported),
    })
}

/// A CAN FD bitrate config string for PCAN-Basic.
///
/// PCAN expresses FD bitrates as `f_clock=...,nom_brp=...` strings.
/// Build one from an [`FdBitrate`] preset, the [`PcanFdBitrate::builder`],
/// or pass a custom string with [`PcanFdBitrate::custom`].
#[derive(Debug, Clone)]
pub struct PcanFdBitrate(pub(crate) std::ffi::CString);

impl PcanFdBitrate {
    /// Wrap an arbitrary PCAN FD bitrate string.
    ///
    /// The string must follow the PCAN format
    /// (`f_clock=80000000,nom_brp=10,...`). The C-string is built
    /// internally; embedded NUL bytes return [`CanError::Param`].
    pub fn custom(s: &str) -> Result<Self, CanError> {
        std::ffi::CString::new(s)
            .map(PcanFdBitrate)
            .map_err(|_| CanError::Param)
    }

    /// Build a config from an [`FdBitrate`] preset, paired with a 500 kbit/s
    /// nominal phase. Returns `Err(CanError::NotSupported)` for any preset
    /// without a known PCAN-compatible string.
    pub fn from_preset(b: FdBitrate) -> Result<Self, CanError> {
        // f_clock=80MHz throughout; nominal 500K@80%; data phase rates are
        // the standard PCAN sample-point settings for 80MHz clocks.
        let s: &str = match b {
            FdBitrate::Rate500K80P =>
                "f_clock=80000000,nom_brp=10,nom_tseg1=12,nom_tseg2=3,nom_sjw=1,\
                 data_brp=10,data_tseg1=12,data_tseg2=3,data_sjw=1",
            FdBitrate::Rate1M80P =>
                "f_clock=80000000,nom_brp=10,nom_tseg1=12,nom_tseg2=3,nom_sjw=1,\
                 data_brp=5,data_tseg1=12,data_tseg2=3,data_sjw=1",
            FdBitrate::Rate2M80P =>
                "f_clock=80000000,nom_brp=10,nom_tseg1=12,nom_tseg2=3,nom_sjw=1,\
                 data_brp=2,data_tseg1=15,data_tseg2=4,data_sjw=4",
            FdBitrate::Rate4M80P =>
                "f_clock=80000000,nom_brp=10,nom_tseg1=12,nom_tseg2=3,nom_sjw=1,\
                 data_brp=2,data_tseg1=7,data_tseg2=2,data_sjw=2",
            FdBitrate::Rate8M80P =>
                "f_clock=80000000,nom_brp=10,nom_tseg1=12,nom_tseg2=3,nom_sjw=1,\
                 data_brp=1,data_tseg1=7,data_tseg2=2,data_sjw=2",
            FdBitrate::Rate8M70P | FdBitrate::Rate8M60P => return Err(CanError::NotSupported),
        };
        Self::custom(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_bitrates_map() {
        assert_eq!(bitrate_to_btr(Bitrate::Rate500K).unwrap(), sys::PCAN_BAUD_500K);
        assert_eq!(bitrate_to_btr(Bitrate::Rate1M).unwrap(), sys::PCAN_BAUD_1M);
    }

    #[test]
    fn rate62k_returns_not_supported() {
        assert_eq!(bitrate_to_btr(Bitrate::Rate62K), Err(CanError::NotSupported));
    }

    #[test]
    fn custom_fd_bitrate_round_trip() {
        let b = PcanFdBitrate::custom("f_clock=80000000,nom_brp=1").unwrap();
        assert_eq!(b.0.to_str().unwrap(), "f_clock=80000000,nom_brp=1");
    }

    #[test]
    fn nul_in_custom_returns_param() {
        assert!(matches!(
            PcanFdBitrate::custom("foo\0bar"),
            Err(CanError::Param)
        ));
    }
}
