//! Translation between [`MessageFlags`](can_core::MessageFlags) and the
//! `TPCANMessageType` byte/word used by PCAN-Basic.

use can_core::MessageFlags;
use pcan_sys as sys;

/// Translate `MessageFlags` into the PCAN message-type byte.
///
/// The `STANDARD` value is 0x00 (absence of `EXTENDED`); we don't emit
/// it explicitly. Vendor-private flags in `MessageFlags` that PCAN has
/// no equivalent for (e.g. `ECHO` when sending) are dropped.
pub(crate) fn flags_to_pcan(flags: MessageFlags) -> sys::TPCANMessageType {
    let mut out: sys::TPCANMessageType = sys::PCAN_MESSAGE_STANDARD;
    if flags.contains(MessageFlags::RTR) { out |= sys::PCAN_MESSAGE_RTR; }
    if flags.contains(MessageFlags::EXT) { out |= sys::PCAN_MESSAGE_EXTENDED; }
    if flags.contains(MessageFlags::FD) { out |= sys::PCAN_MESSAGE_FD; }
    if flags.contains(MessageFlags::BRS) { out |= sys::PCAN_MESSAGE_BRS; }
    if flags.contains(MessageFlags::ESI) { out |= sys::PCAN_MESSAGE_ESI; }
    out
}

/// Translate a PCAN message-type byte into [`MessageFlags`].
///
/// Sets `STD` when neither `EXTENDED` is present. Status frames
/// (`PCAN_MESSAGE_STATUS`) and echoes (`PCAN_MESSAGE_ECHO`) are surfaced
/// via the `ECHO` flag and the `ERROR_FRAME` flag respectively when a
/// status frame indicates a bus condition; callers above this layer
/// translate further.
pub(crate) fn flags_from_pcan(msg_type: sys::TPCANMessageType) -> MessageFlags {
    let mut out = MessageFlags::empty();
    if msg_type & sys::PCAN_MESSAGE_EXTENDED != 0 {
        out |= MessageFlags::EXT;
    } else {
        out |= MessageFlags::STD;
    }
    if msg_type & sys::PCAN_MESSAGE_RTR != 0 { out |= MessageFlags::RTR; }
    if msg_type & sys::PCAN_MESSAGE_FD != 0 { out |= MessageFlags::FD; }
    if msg_type & sys::PCAN_MESSAGE_BRS != 0 { out |= MessageFlags::BRS; }
    if msg_type & sys::PCAN_MESSAGE_ESI != 0 { out |= MessageFlags::ESI; }
    if msg_type & sys::PCAN_MESSAGE_ECHO != 0 { out |= MessageFlags::ECHO; }
    if msg_type & sys::PCAN_MESSAGE_ERRFRAME != 0 { out |= MessageFlags::ERROR_FRAME; }
    out
}

/// FD DLC code → byte length.
///
/// CAN FD encodes DLC as a 4-bit nibble where 9..15 mean 12, 16, 20, 24,
/// 32, 48, 64 bytes respectively.
pub(crate) fn fd_dlc_to_len(dlc: u8) -> usize {
    match dlc & 0x0F {
        0..=8 => dlc as usize,
        9 => 12,
        10 => 16,
        11 => 20,
        12 => 24,
        13 => 32,
        14 => 48,
        15 => 64,
        _ => unreachable!(),
    }
}

/// FD byte length → DLC code.
pub(crate) fn fd_len_to_dlc(len: usize) -> u8 {
    match len {
        0..=8 => len as u8,
        9..=12 => 9,
        13..=16 => 10,
        17..=20 => 11,
        21..=24 => 12,
        25..=32 => 13,
        33..=48 => 14,
        49..=64 => 15,
        _ => 15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_flags_round_trip() {
        let f = MessageFlags::STD;
        let pcan = flags_to_pcan(f);
        let back = flags_from_pcan(pcan);
        assert!(back.contains(MessageFlags::STD));
    }

    #[test]
    fn extended_flag_round_trips() {
        let pcan = flags_to_pcan(MessageFlags::EXT);
        assert_eq!(pcan & sys::PCAN_MESSAGE_EXTENDED, sys::PCAN_MESSAGE_EXTENDED);
        let back = flags_from_pcan(pcan);
        assert!(back.contains(MessageFlags::EXT));
        assert!(!back.contains(MessageFlags::STD));
    }

    #[test]
    fn fd_brs_flags_round_trip() {
        let f = MessageFlags::EXT | MessageFlags::FD | MessageFlags::BRS;
        let pcan = flags_to_pcan(f);
        let back = flags_from_pcan(pcan);
        assert!(back.contains(MessageFlags::EXT));
        assert!(back.contains(MessageFlags::FD));
        assert!(back.contains(MessageFlags::BRS));
    }

    #[test]
    fn fd_dlc_table_matches_spec() {
        assert_eq!(fd_dlc_to_len(0), 0);
        assert_eq!(fd_dlc_to_len(8), 8);
        assert_eq!(fd_dlc_to_len(9), 12);
        assert_eq!(fd_dlc_to_len(15), 64);

        assert_eq!(fd_len_to_dlc(0), 0);
        assert_eq!(fd_len_to_dlc(8), 8);
        assert_eq!(fd_len_to_dlc(12), 9);
        assert_eq!(fd_len_to_dlc(64), 15);
    }
}
