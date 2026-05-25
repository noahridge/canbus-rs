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
/// Sets `STD` when `EXTENDED` is absent. Both error frames
/// (`PCAN_MESSAGE_ERRFRAME`) and status frames (`PCAN_MESSAGE_STATUS`)
/// map to `ERROR_FRAME` so the message is dispatched as
/// [`CanMessage::Error`](can_core::CanMessage) rather than being
/// misread as a data frame; echoes map to `ECHO`. The specific
/// protocol-error kind (overrun, etc.) is decoded from the status
/// payload by [`read_flags`].
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
    if msg_type & (sys::PCAN_MESSAGE_ERRFRAME | sys::PCAN_MESSAGE_STATUS) != 0 {
        out |= MessageFlags::ERROR_FRAME;
    }
    out
}

/// Decode the payload of a PCAN status frame into error `MessageFlags`.
///
/// A status frame carries a four-byte, big-endian [`TPCANStatus`] value in
/// its data field. Only overrun maps cleanly onto a `MessageFlags` bit;
/// bus-state conditions (bus-off, error-passive) are reported through
/// [`read_status`](crate::channel::PcanChannel::read_status)/`BusStatus`,
/// so they add no flag here and the frame decodes to `ErrorKind::Other`.
///
/// Returns `empty()` for payloads shorter than four bytes.
fn status_payload_to_err_flags(data: &[u8]) -> MessageFlags {
    if data.len() < 4 {
        return MessageFlags::empty();
    }
    let status = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let mut out = MessageFlags::empty();
    if status & (sys::PCAN_ERROR_OVERRUN | sys::PCAN_ERROR_QOVERRUN) != 0 {
        out |= MessageFlags::ERR_HW_OVERRUN;
    }
    out
}

/// Build the `MessageFlags` for a received frame from its message-type byte
/// and (for status frames) its payload.
///
/// This is the entry point the channel uses on receive; it layers the
/// status-payload decode on top of [`flags_from_pcan`].
pub(crate) fn read_flags(msg_type: sys::TPCANMessageType, data: &[u8]) -> MessageFlags {
    let mut flags = flags_from_pcan(msg_type);
    if msg_type & sys::PCAN_MESSAGE_STATUS != 0 {
        flags |= status_payload_to_err_flags(data);
    }
    flags
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

    #[test]
    fn status_and_error_frames_map_to_error_frame() {
        // A status frame must not be mistaken for a data frame.
        assert!(flags_from_pcan(sys::PCAN_MESSAGE_STATUS).contains(MessageFlags::ERROR_FRAME));
        assert!(flags_from_pcan(sys::PCAN_MESSAGE_ERRFRAME).contains(MessageFlags::ERROR_FRAME));
    }

    #[test]
    fn read_flags_decodes_overrun_from_status_payload() {
        // Big-endian PCAN_ERROR_QOVERRUN (0x40) in the 4-byte status payload.
        let payload = (sys::PCAN_ERROR_QOVERRUN).to_be_bytes();
        let flags = read_flags(sys::PCAN_MESSAGE_STATUS, &payload);
        assert!(flags.contains(MessageFlags::ERROR_FRAME));
        assert!(flags.contains(MessageFlags::ERR_HW_OVERRUN));
    }

    #[test]
    fn read_flags_status_without_overrun_has_no_err_bits() {
        // Bus-off status: surfaced as an error frame, but no protocol-error
        // bit (bus state is reported via read_status instead).
        let payload = (sys::PCAN_ERROR_BUSOFF).to_be_bytes();
        let flags = read_flags(sys::PCAN_MESSAGE_STATUS, &payload);
        assert!(flags.contains(MessageFlags::ERROR_FRAME));
        assert!(!flags.contains(MessageFlags::ERR_HW_OVERRUN));
    }

    #[test]
    fn read_flags_data_frame_is_not_an_error_frame() {
        let flags = read_flags(sys::PCAN_MESSAGE_STANDARD, &[1, 2, 3, 4]);
        assert!(flags.contains(MessageFlags::STD));
        assert!(!flags.contains(MessageFlags::ERROR_FRAME));
    }
}
