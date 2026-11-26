//! Translation between `canbus-core` types and PCAN-Basic's native values.

use std::os::raw::c_char;

use canbus_core::{BusConfig, BusState, Error, MessageFlags, Result, Timing};

use crate::NAME;
use crate::ffi as sys;

/// Get the loaded PCAN-Basic, or [`Error::LibraryNotLoaded`].
pub(crate) fn lib() -> Result<&'static sys::PcanLib> {
    sys::get().map_err(|e| Error::LibraryNotLoaded {
        interface: NAME,
        reason: e.to_string(),
    })
}

/// `PCAN_ERROR_OK`, ignoring the informational "caution" bit.
pub(crate) fn is_ok(status: sys::TPCANStatus) -> bool {
    status & !sys::PCAN_ERROR_CAUTION == sys::PCAN_ERROR_OK
}

/// Turn a `TPCANStatus` into a `Result`.
pub(crate) fn check(lib: &sys::PcanLib, status: sys::TPCANStatus) -> Result<()> {
    if is_ok(status) {
        Ok(())
    } else {
        Err(error_from_status(lib, status & !sys::PCAN_ERROR_CAUTION))
    }
}

/// Bits 10-12 of a status hold one of several handle errors as a value,
/// not as independent flags.
const HANDLE_ERROR_FIELD: sys::TPCANStatus = 0x1C00;

/// Map a non-OK `TPCANStatus` onto the vendor-neutral [`Error`].
pub(crate) fn error_from_status(lib: &sys::PcanLib, status: sys::TPCANStatus) -> Error {
    match status & HANDLE_ERROR_FIELD {
        sys::PCAN_ERROR_HWINUSE | sys::PCAN_ERROR_NETINUSE => return Error::ChannelInUse,
        sys::PCAN_ERROR_ILLHW | sys::PCAN_ERROR_ILLNET | sys::PCAN_ERROR_ILLCLIENT => {
            return Error::ChannelNotFound;
        }
        _ => {}
    }
    if status & (sys::PCAN_ERROR_OVERRUN | sys::PCAN_ERROR_QOVERRUN) != 0 {
        Error::Overrun
    } else if status & (sys::PCAN_ERROR_XMTFULL | sys::PCAN_ERROR_QXMTFULL) != 0 {
        Error::TxQueueFull
    } else if status & sys::PCAN_ERROR_BUSOFF != 0 {
        Error::BusOff
    } else if status & (sys::PCAN_ERROR_ILLOPERATION | sys::PCAN_ERROR_ILLMODE) != 0 {
        Error::Unsupported("not supported by this PCAN device or mode")
    } else if status & sys::PCAN_ERROR_REGTEST != 0 {
        Error::Hardware
    } else if status & sys::PCAN_ERROR_NODRIVER != 0 {
        Error::Driver
    } else {
        Error::Backend {
            interface: NAME,
            code: status.into(),
            message: error_text(lib, status),
        }
    }
}

/// PCAN-Basic's (English) description of a status code.
fn error_text(lib: &sys::PcanLib, status: sys::TPCANStatus) -> String {
    const ENGLISH: u16 = 0x09;
    // PCAN-Basic requires a buffer of at least 256 bytes.
    let mut buf = [0u8; 256];
    // SAFETY: the buffer meets the documented minimum size.
    let rc = unsafe { (lib.CAN_GetErrorText)(status, ENGLISH, buf.as_mut_ptr() as *mut c_char) };
    if !is_ok(rc) {
        return format!("PCAN status 0x{status:X}");
    }
    c_buf_to_string(&buf)
}

/// Read a NUL-terminated byte buffer as (lossy) UTF-8.
pub(crate) fn c_buf_to_string(buf: &[u8]) -> String {
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

/// Map a `CAN_GetStatus` result to a [`BusState`].
///
/// `CAN_GetStatus` reports bus-state bits only; any other bit pattern
/// (e.g. `PCAN_ERROR_QRCVEMPTY`) means the bus itself is fine.
pub(crate) fn bus_state_from_pcan(status: sys::TPCANStatus) -> BusState {
    if status & sys::PCAN_ERROR_BUSOFF != 0 {
        BusState::BusOff
    } else if status & sys::PCAN_ERROR_BUSPASSIVE != 0 {
        BusState::Passive
    } else if status & (sys::PCAN_ERROR_BUSWARNING | sys::PCAN_ERROR_BUSLIGHT) != 0 {
        BusState::Warning
    } else {
        BusState::Active
    }
}

/// Translate `MessageFlags` into the PCAN message-type byte.
///
/// `STANDARD` is 0x00 (absence of `EXTENDED`). Flags PCAN has no transmit
/// equivalent for (e.g. `ECHO`) are dropped.
pub(crate) fn flags_to_pcan(flags: MessageFlags) -> sys::TPCANMessageType {
    let mut out = sys::PCAN_MESSAGE_STANDARD;
    if flags.contains(MessageFlags::RTR) {
        out |= sys::PCAN_MESSAGE_RTR;
    }
    if flags.contains(MessageFlags::EXT) {
        out |= sys::PCAN_MESSAGE_EXTENDED;
    }
    if flags.contains(MessageFlags::FD) {
        out |= sys::PCAN_MESSAGE_FD;
    }
    if flags.contains(MessageFlags::BRS) {
        out |= sys::PCAN_MESSAGE_BRS;
    }
    if flags.contains(MessageFlags::ESI) {
        out |= sys::PCAN_MESSAGE_ESI;
    }
    out
}

/// Translate a PCAN message-type byte into [`MessageFlags`].
///
/// Both error frames and status frames map to `ERROR_FRAME` so they surface
/// as [`CanMessage::Error`](canbus_core::CanMessage) instead of being misread
/// as data; [`read_flags`] adds the decoded status payload.
pub(crate) fn flags_from_pcan(msg_type: sys::TPCANMessageType) -> MessageFlags {
    let mut out = if msg_type & sys::PCAN_MESSAGE_EXTENDED != 0 {
        MessageFlags::EXT
    } else {
        MessageFlags::STD
    };
    for (bit, flag) in [
        (sys::PCAN_MESSAGE_RTR, MessageFlags::RTR),
        (sys::PCAN_MESSAGE_FD, MessageFlags::FD),
        (sys::PCAN_MESSAGE_BRS, MessageFlags::BRS),
        (sys::PCAN_MESSAGE_ESI, MessageFlags::ESI),
        (sys::PCAN_MESSAGE_ECHO, MessageFlags::ECHO),
    ] {
        if msg_type & bit != 0 {
            out |= flag;
        }
    }
    if msg_type & (sys::PCAN_MESSAGE_ERRFRAME | sys::PCAN_MESSAGE_STATUS) != 0 {
        out |= MessageFlags::ERROR_FRAME;
    }
    out
}

/// Flags for a received frame: the message-type byte plus, for status
/// frames, the overrun bit decoded from the 4-byte big-endian payload.
pub(crate) fn read_flags(msg_type: sys::TPCANMessageType, data: &[u8]) -> MessageFlags {
    let mut flags = flags_from_pcan(msg_type);
    if msg_type & sys::PCAN_MESSAGE_STATUS != 0 && data.len() >= 4 {
        let status = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if status & (sys::PCAN_ERROR_OVERRUN | sys::PCAN_ERROR_QOVERRUN) != 0 {
            flags |= MessageFlags::ERR_HW_OVERRUN;
        }
    }
    flags
}

/// PCAN's predefined BTR0/BTR1 code for a classic bitrate, if any.
fn classic_preset(bitrate: u32) -> Option<sys::TPCANBaudrate> {
    Some(match bitrate {
        1_000_000 => sys::PCAN_BAUD_1M,
        800_000 => sys::PCAN_BAUD_800K,
        500_000 => sys::PCAN_BAUD_500K,
        250_000 => sys::PCAN_BAUD_250K,
        125_000 => sys::PCAN_BAUD_125K,
        100_000 => sys::PCAN_BAUD_100K,
        95_000 | 95_238 => sys::PCAN_BAUD_95K,
        83_000 | 83_333 => sys::PCAN_BAUD_83K,
        50_000 => sys::PCAN_BAUD_50K,
        47_000 | 47_619 => sys::PCAN_BAUD_47K,
        33_000 | 33_333 => sys::PCAN_BAUD_33K,
        20_000 => sys::PCAN_BAUD_20K,
        10_000 => sys::PCAN_BAUD_10K,
        5_000 => sys::PCAN_BAUD_5K,
        _ => return None,
    })
}

/// Encode classic timing as SJA1000 BTR0/BTR1 registers (8 MHz clock).
fn timing_to_btr(t: &Timing) -> Option<sys::TPCANBaudrate> {
    let valid = t.clock_hz == 8_000_000
        && (1..=64).contains(&t.brp)
        && (1..=16).contains(&t.tseg1)
        && (1..=8).contains(&t.tseg2)
        && (1..=4).contains(&t.sjw);
    if !valid {
        return None;
    }
    let btr0 = ((t.sjw - 1) << 6) | (t.brp - 1);
    let btr1 = ((t.tseg2 - 1) << 4) | (t.tseg1 - 1);
    Some(((btr0 << 8) | btr1) as sys::TPCANBaudrate)
}

/// The BTR0/BTR1 code for a classic-CAN `CAN_Initialize`.
pub(crate) fn classic_btr(config: &BusConfig) -> Result<sys::TPCANBaudrate> {
    if let Some(t) = &config.timing {
        return timing_to_btr(t).ok_or_else(|| {
            Error::InvalidConfig(
                "PCAN classic timing must use an 8 MHz clock with brp 1-64, tseg1 1-16, \
                 tseg2 1-8 and sjw 1-4"
                    .into(),
            )
        });
    }
    let bitrate = config
        .bitrate
        .ok_or_else(|| Error::InvalidConfig("PCAN requires a bitrate".into()))?;
    classic_preset(bitrate).ok_or_else(|| {
        Error::InvalidConfig(format!(
            "no PCAN preset for bitrate {bitrate}; set explicit timing instead"
        ))
    })
}

/// 80 MHz FD clock timings: (bitrate, brp, tseg1, tseg2, sjw).
const FD_NOMINAL: [(u32, u32, u32, u32, u32); 4] = [
    (125_000, 40, 12, 3, 1),
    (250_000, 20, 12, 3, 1),
    (500_000, 10, 12, 3, 1),
    (1_000_000, 5, 12, 3, 1),
];
const FD_DATA: [(u32, u32, u32, u32, u32); 5] = [
    (500_000, 10, 12, 3, 1),
    (1_000_000, 5, 12, 3, 1),
    (2_000_000, 2, 15, 4, 4),
    (4_000_000, 2, 7, 2, 2),
    (8_000_000, 1, 7, 2, 2),
];
const FD_CLOCK_HZ: u32 = 80_000_000;

fn fd_preset(table: &[(u32, u32, u32, u32, u32)], bitrate: u32) -> Option<Timing> {
    table
        .iter()
        .find(|row| row.0 == bitrate)
        .map(|&(_, brp, tseg1, tseg2, sjw)| Timing {
            clock_hz: FD_CLOCK_HZ,
            brp,
            tseg1,
            tseg2,
            sjw,
        })
}

/// The `CAN_InitializeFD` bitrate string for an FD configuration.
pub(crate) fn fd_bitrate_string(config: &BusConfig) -> Result<String> {
    let fd = config
        .fd
        .ok_or_else(|| Error::InvalidConfig("CAN FD is not enabled".into()))?;
    let nominal = match config.timing {
        Some(t) => t,
        None => {
            let bitrate = config
                .bitrate
                .ok_or_else(|| Error::InvalidConfig("PCAN requires a bitrate".into()))?;
            fd_preset(&FD_NOMINAL, bitrate).ok_or_else(|| {
                Error::InvalidConfig(format!(
                    "no PCAN FD preset for nominal bitrate {bitrate}; presets are 125k, 250k, \
                     500k and 1M, otherwise set explicit timing"
                ))
            })?
        }
    };
    let data = match fd.data_timing {
        Some(t) => t,
        None => fd_preset(&FD_DATA, fd.data_bitrate).ok_or_else(|| {
            Error::InvalidConfig(format!(
                "no PCAN FD preset for data bitrate {}; presets are 500k, 1M, 2M, 4M and 8M, \
                 otherwise set explicit data timing",
                fd.data_bitrate
            ))
        })?,
    };
    if nominal.clock_hz != data.clock_hz {
        return Err(Error::InvalidConfig(
            "PCAN FD nominal and data timing must use the same clock".into(),
        ));
    }
    Ok(format!(
        "f_clock={},nom_brp={},nom_tseg1={},nom_tseg2={},nom_sjw={},\
         data_brp={},data_tseg1={},data_tseg2={},data_sjw={}",
        nominal.clock_hz,
        nominal.brp,
        nominal.tseg1,
        nominal.tseg2,
        nominal.sjw,
        data.brp,
        data.tseg1,
        data.tseg2,
        data.sjw
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_round_trip() {
        for f in [
            MessageFlags::STD,
            MessageFlags::EXT,
            MessageFlags::EXT | MessageFlags::FD | MessageFlags::BRS,
            MessageFlags::STD | MessageFlags::RTR,
        ] {
            assert_eq!(flags_from_pcan(flags_to_pcan(f)), f);
        }
    }

    #[test]
    fn status_and_error_frames_map_to_error_frame() {
        assert!(flags_from_pcan(sys::PCAN_MESSAGE_STATUS).contains(MessageFlags::ERROR_FRAME));
        assert!(flags_from_pcan(sys::PCAN_MESSAGE_ERRFRAME).contains(MessageFlags::ERROR_FRAME));
        assert!(flags_from_pcan(sys::PCAN_MESSAGE_ECHO).contains(MessageFlags::ECHO));
    }

    #[test]
    fn status_payload_overrun() {
        let payload = sys::PCAN_ERROR_QOVERRUN.to_be_bytes();
        let flags = read_flags(sys::PCAN_MESSAGE_STATUS, &payload);
        assert!(flags.contains(MessageFlags::ERROR_FRAME | MessageFlags::ERR_HW_OVERRUN));
        let bus_off = sys::PCAN_ERROR_BUSOFF.to_be_bytes();
        assert!(
            !read_flags(sys::PCAN_MESSAGE_STATUS, &bus_off).contains(MessageFlags::ERR_HW_OVERRUN)
        );
        assert!(
            !read_flags(sys::PCAN_MESSAGE_STANDARD, &[1, 2, 3, 4])
                .contains(MessageFlags::ERROR_FRAME)
        );
    }

    #[test]
    fn bus_state_mapping() {
        assert_eq!(bus_state_from_pcan(sys::PCAN_ERROR_OK), BusState::Active);
        assert_eq!(
            bus_state_from_pcan(sys::PCAN_ERROR_QRCVEMPTY),
            BusState::Active
        );
        assert_eq!(
            bus_state_from_pcan(sys::PCAN_ERROR_BUSOFF),
            BusState::BusOff
        );
        assert_eq!(
            bus_state_from_pcan(sys::PCAN_ERROR_BUSPASSIVE),
            BusState::Passive
        );
        assert_eq!(
            bus_state_from_pcan(sys::PCAN_ERROR_BUSWARNING),
            BusState::Warning
        );
    }

    #[test]
    fn handle_error_field_is_compared_as_a_value() {
        // HWINUSE (0x400) shares bits with ILLHW (0x1400); a bitwise test
        // would misclassify it.
        for (status, in_use) in [
            (sys::PCAN_ERROR_HWINUSE, true),
            (sys::PCAN_ERROR_NETINUSE, true),
            (sys::PCAN_ERROR_ILLHW, false),
            (sys::PCAN_ERROR_ILLNET, false),
            (sys::PCAN_ERROR_ILLCLIENT, false),
        ] {
            let field = status & HANDLE_ERROR_FIELD;
            let is_in_use = field == sys::PCAN_ERROR_HWINUSE || field == sys::PCAN_ERROR_NETINUSE;
            assert_eq!(is_in_use, in_use, "0x{status:X}");
        }
        assert!(is_ok(sys::PCAN_ERROR_CAUTION));
    }

    #[test]
    fn classic_presets_and_timing() {
        let cfg = BusConfig::new(NAME, "PCAN_USBBUS1").bitrate(500_000);
        assert_eq!(classic_btr(&cfg), Ok(sys::PCAN_BAUD_500K));
        assert!(classic_btr(&BusConfig::new(NAME, "x").bitrate(123_456)).is_err());
        assert!(classic_btr(&BusConfig::new(NAME, "x")).is_err());

        // PCAN_BAUD_500K is 0x001C: sjw 1, brp 1, tseg1 13, tseg2 2 at 8 MHz.
        let t = Timing {
            clock_hz: 8_000_000,
            brp: 1,
            tseg1: 13,
            tseg2: 2,
            sjw: 1,
        };
        assert_eq!(t.bitrate(), 500_000);
        let cfg = BusConfig::new(NAME, "x").timing(t);
        assert_eq!(classic_btr(&cfg), Ok(sys::PCAN_BAUD_500K));
        let wrong_clock = BusConfig::new(NAME, "x").timing(Timing {
            clock_hz: 16_000_000,
            ..t
        });
        assert!(classic_btr(&wrong_clock).is_err());
    }

    #[test]
    fn fd_strings() {
        let cfg = BusConfig::new(NAME, "x").bitrate(500_000).fd(2_000_000);
        assert_eq!(
            fd_bitrate_string(&cfg).unwrap(),
            "f_clock=80000000,nom_brp=10,nom_tseg1=12,nom_tseg2=3,nom_sjw=1,\
             data_brp=2,data_tseg1=15,data_tseg2=4,data_sjw=4"
        );
        for (_, brp, tseg1, tseg2, sjw) in FD_NOMINAL.iter().chain(FD_DATA.iter()) {
            let t = Timing {
                clock_hz: FD_CLOCK_HZ,
                brp: *brp,
                tseg1: *tseg1,
                tseg2: *tseg2,
                sjw: *sjw,
            };
            assert!(
                FD_NOMINAL
                    .iter()
                    .chain(FD_DATA.iter())
                    .any(|r| r.0 == t.bitrate())
            );
        }
        assert!(
            fd_bitrate_string(&BusConfig::new(NAME, "x").bitrate(500_000).fd(3_000_000)).is_err()
        );
        assert!(
            fd_bitrate_string(&BusConfig::new(NAME, "x").bitrate(800_000).fd(2_000_000)).is_err()
        );
    }
}
