//! CAN frame type and flags, vendor-neutral.
//!
//! [`MessageFlags`] uses bit positions chosen for this crate; each backend
//! translates to/from its native flag representation at the FFI boundary.

use core::time::Duration;

use embedded_can::{ExtendedId, Id, StandardId};

use crate::error::{Error, Result};

/// CAN FD Bit Rate Switch.
///
/// Controls whether the data phase of a CAN FD frame is transmitted at
/// the faster FD bitrate (`On`) or at the arbitration bitrate (`Off`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brs {
    /// Switch to the FD data-phase bitrate during the data field.
    On,
    /// Stay at the arbitration-phase bitrate.
    Off,
}

bitflags::bitflags! {
    /// Flags associated with a CAN message.
    ///
    /// The bit values are crate-defined and do **not** match any vendor's
    /// representation. Backends translate when crossing the FFI boundary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MessageFlags: u32 {
        /// Remote Transmission Request.
        const RTR = 0x0000_0001;
        /// Standard (11-bit) identifier.
        const STD = 0x0000_0002;
        /// Extended (29-bit) identifier.
        const EXT = 0x0000_0004;
        /// Error frame.
        const ERROR_FRAME = 0x0000_0008;
        /// CAN FD frame.
        const FD = 0x0000_0010;
        /// CAN FD Bit Rate Switch.
        const BRS = 0x0000_0020;
        /// CAN FD Error State Indicator.
        const ESI = 0x0000_0040;
        /// Echo of a frame this bus transmitted.
        const ECHO = 0x0000_0080;
        /// Hardware receive buffer overrun (set on error frames).
        const ERR_HW_OVERRUN = 0x0000_0100;
        /// Software receive buffer overrun (set on error frames).
        const ERR_SW_OVERRUN = 0x0000_0200;
        /// Stuff error (set on error frames).
        const ERR_STUFF = 0x0000_0400;
        /// Form error (set on error frames).
        const ERR_FORM = 0x0000_0800;
        /// CRC error (set on error frames).
        const ERR_CRC = 0x0000_1000;
        /// Bit-0 error (dominant transmitted, recessive monitored).
        const ERR_BIT0 = 0x0000_2000;
        /// Bit-1 error (recessive transmitted, dominant monitored).
        const ERR_BIT1 = 0x0000_4000;
    }
}

/// Maximum payload length for classic CAN.
pub const CAN_MAX_LEN: usize = 8;

/// Maximum payload length for CAN FD.
pub const CANFD_MAX_LEN: usize = 64;

/// Maximum standard (11-bit) CAN identifier.
pub const CAN_STD_ID_MAX: u32 = 0x7FF;

/// Maximum extended (29-bit) CAN identifier.
pub const CAN_EXT_ID_MAX: u32 = 0x1FFF_FFFF;

/// Payload lengths for CAN FD DLC codes 9..=15.
const FD_LENGTHS: [usize; 7] = [12, 16, 20, 24, 32, 48, 64];

/// Round a payload length up to the nearest valid CAN FD frame length
/// (0-8, 12, 16, 20, 24, 32, 48 or 64). Lengths above 64 return 64.
pub fn fd_len_round_up(len: usize) -> usize {
    if len <= CAN_MAX_LEN {
        return len;
    }
    FD_LENGTHS
        .iter()
        .copied()
        .find(|&valid| len <= valid)
        .unwrap_or(CANFD_MAX_LEN)
}

/// Convert a payload length in bytes to the 4-bit DLC code (0..=15).
///
/// Lengths that aren't a valid FD length are rounded up first.
pub fn len_to_dlc(len: usize) -> u8 {
    let len = fd_len_round_up(len);
    if len <= CAN_MAX_LEN {
        len as u8
    } else {
        // `fd_len_round_up` only returns values present in FD_LENGTHS.
        9 + FD_LENGTHS.iter().position(|&l| l == len).unwrap_or(6) as u8
    }
}

/// Convert a 4-bit DLC code to the payload length in bytes.
///
/// Only the low nibble is considered. For classic CAN, codes 9..=15 still
/// mean an 8-byte payload; pass `fd = false` to get that interpretation.
pub fn dlc_to_len(dlc: u8, fd: bool) -> usize {
    let dlc = (dlc & 0x0F) as usize;
    match dlc {
        0..=8 => dlc,
        _ if fd => FD_LENGTHS[dlc - 9],
        _ => CAN_MAX_LEN,
    }
}

/// A CAN data frame (classic or FD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFrame {
    pub(crate) id: u32,
    pub(crate) data: Vec<u8>,
    pub(crate) flags: MessageFlags,
    pub(crate) timestamp: Option<Duration>,
}

/// A CAN Remote Transmission Request frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFrame {
    pub(crate) id: u32,
    pub(crate) len: u8,
    pub(crate) flags: MessageFlags,
    pub(crate) timestamp: Option<Duration>,
}

/// A CAN error frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorFrame {
    pub(crate) id: u32,
    pub(crate) data: Vec<u8>,
    pub(crate) flags: MessageFlags,
    pub(crate) timestamp: Option<Duration>,
}

/// A CAN message (classic or FD).
///
/// Each variant enforces which fields and flag combinations are valid:
/// - `Data` — data frames (classic CAN and FD)
/// - `Remote` — Remote Transmission Request frames (no payload)
/// - `Error` — error frames reported by the controller
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanMessage {
    /// A data frame (classic CAN or CAN FD).
    Data(DataFrame),
    /// A Remote Transmission Request frame.
    Remote(RemoteFrame),
    /// An error frame.
    Error(ErrorFrame),
}

/// Convert a typed [`Id`] into the raw `u32` and matching std/ext flag.
pub fn id_to_raw_and_flag(id: Id) -> (u32, MessageFlags) {
    match id {
        Id::Standard(s) => (u32::from(s.as_raw()), MessageFlags::STD),
        Id::Extended(e) => (e.as_raw(), MessageFlags::EXT),
    }
}

impl CanMessage {
    /// Create a classic CAN data frame.
    ///
    /// The `id` may be a [`StandardId`], an [`ExtendedId`] or an [`Id`];
    /// the std/ext flag is taken from its variant.
    ///
    /// Returns [`Error::InvalidFrame`] if `data.len() > 8`.
    pub fn new(id: impl Into<Id>, data: &[u8]) -> Result<Self> {
        if data.len() > CAN_MAX_LEN {
            return Err(Error::InvalidFrame("classic CAN payload exceeds 8 bytes"));
        }
        let (raw, flag) = id_to_raw_and_flag(id.into());
        Ok(CanMessage::Data(DataFrame {
            id: raw,
            data: data.to_vec(),
            flags: flag,
            timestamp: None,
        }))
    }

    /// Create a CAN FD data frame.
    ///
    /// The payload is zero-padded up to the next valid CAN FD length
    /// (0-8, 12, 16, 20, 24, 32, 48 or 64). Pass [`Brs::On`] to switch to the
    /// data-phase bitrate during the data field.
    ///
    /// Returns [`Error::InvalidFrame`] if `data.len() > 64`.
    pub fn new_fd(id: impl Into<Id>, data: &[u8], brs: Brs) -> Result<Self> {
        if data.len() > CANFD_MAX_LEN {
            return Err(Error::InvalidFrame("CAN FD payload exceeds 64 bytes"));
        }
        let mut padded = vec![0u8; fd_len_round_up(data.len())];
        padded[..data.len()].copy_from_slice(data);

        let (raw, mut flags) = id_to_raw_and_flag(id.into());
        flags |= MessageFlags::FD;
        if brs == Brs::On {
            flags |= MessageFlags::BRS;
        }
        Ok(CanMessage::Data(DataFrame {
            id: raw,
            data: padded,
            flags,
            timestamp: None,
        }))
    }

    /// Create a Remote Transmission Request frame requesting `len` bytes.
    ///
    /// Returns [`Error::InvalidFrame`] if `len > 8`.
    pub fn new_remote(id: impl Into<Id>, len: u8) -> Result<Self> {
        if len as usize > CAN_MAX_LEN {
            return Err(Error::InvalidFrame("remote frame length exceeds 8 bytes"));
        }
        let (raw, flag) = id_to_raw_and_flag(id.into());
        Ok(CanMessage::Remote(RemoteFrame {
            id: raw,
            len,
            flags: flag | MessageFlags::RTR,
            timestamp: None,
        }))
    }

    /// Build a received message from raw parts, dispatching on `flags`.
    ///
    /// Intended for backend implementations. No validation is performed:
    /// - `ERROR_FRAME` → [`CanMessage::Error`]
    /// - `RTR` → [`CanMessage::Remote`] (requesting `len` bytes; `data` ignored)
    /// - otherwise → [`CanMessage::Data`] (payload is `data`; `len` ignored)
    pub fn from_raw(
        id: u32,
        data: &[u8],
        len: u8,
        flags: MessageFlags,
        timestamp: Option<Duration>,
    ) -> Self {
        if flags.contains(MessageFlags::ERROR_FRAME) {
            CanMessage::Error(ErrorFrame {
                id,
                data: data.to_vec(),
                flags,
                timestamp,
            })
        } else if flags.contains(MessageFlags::RTR) {
            CanMessage::Remote(RemoteFrame {
                id,
                len,
                flags,
                timestamp,
            })
        } else {
            CanMessage::Data(DataFrame {
                id,
                data: data.to_vec(),
                flags,
                timestamp,
            })
        }
    }

    /// Raw CAN identifier (11-bit or 29-bit; see [`Self::is_extended`]).
    pub fn id(&self) -> u32 {
        match self {
            CanMessage::Data(f) => f.id,
            CanMessage::Remote(f) => f.id,
            CanMessage::Error(f) => f.id,
        }
    }

    /// Typed CAN identifier, or `None` if the raw value is out of range for
    /// its frame format (only possible for malformed received frames).
    pub fn typed_id(&self) -> Option<Id> {
        let raw = self.id();
        if self.is_extended() {
            ExtendedId::new(raw).map(Id::Extended)
        } else {
            u16::try_from(raw)
                .ok()
                .and_then(StandardId::new)
                .map(Id::Standard)
        }
    }

    /// Message payload. Empty for remote frames.
    pub fn data(&self) -> &[u8] {
        match self {
            CanMessage::Data(f) => &f.data,
            CanMessage::Remote(_) => &[],
            CanMessage::Error(f) => &f.data,
        }
    }

    /// Payload length in bytes. For remote frames, the requested length.
    pub fn len(&self) -> usize {
        match self {
            CanMessage::Remote(f) => f.len as usize,
            _ => self.data().len(),
        }
    }

    /// True if [`Self::len`] is zero.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The 4-bit Data Length Code encoding [`Self::len`].
    pub fn dlc(&self) -> u8 {
        len_to_dlc(self.len())
    }

    /// Message flags.
    pub fn flags(&self) -> MessageFlags {
        match self {
            CanMessage::Data(f) => f.flags,
            CanMessage::Remote(f) => f.flags,
            CanMessage::Error(f) => f.flags,
        }
    }

    /// Receive timestamp, set by the backend on received frames.
    ///
    /// The epoch is backend-defined (typically when the channel was opened
    /// or the device booted); only differences between timestamps from the
    /// same bus are meaningful.
    pub fn timestamp(&self) -> Option<Duration> {
        match self {
            CanMessage::Data(f) => f.timestamp,
            CanMessage::Remote(f) => f.timestamp,
            CanMessage::Error(f) => f.timestamp,
        }
    }

    /// Return a copy of this message with the given timestamp.
    pub fn with_timestamp(mut self, timestamp: Option<Duration>) -> Self {
        match &mut self {
            CanMessage::Data(f) => f.timestamp = timestamp,
            CanMessage::Remote(f) => f.timestamp = timestamp,
            CanMessage::Error(f) => f.timestamp = timestamp,
        }
        self
    }

    /// True for CAN FD frames.
    pub fn is_fd(&self) -> bool {
        self.flags().contains(MessageFlags::FD)
    }

    /// True for extended (29-bit) frames.
    pub fn is_extended(&self) -> bool {
        self.flags().contains(MessageFlags::EXT)
    }

    /// True for Remote Transmission Request frames.
    pub fn is_remote(&self) -> bool {
        matches!(self, CanMessage::Remote(_))
    }

    /// True for error frames.
    pub fn is_error_frame(&self) -> bool {
        matches!(self, CanMessage::Error(_))
    }

    /// True if this is an echo of a frame this bus transmitted.
    pub fn is_echo(&self) -> bool {
        self.flags().contains(MessageFlags::ECHO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std(id: u16) -> StandardId {
        StandardId::new(id).expect("test id in range")
    }
    fn ext(id: u32) -> ExtendedId {
        ExtendedId::new(id).expect("test id in range")
    }

    #[test]
    fn new_sets_std_flag_and_len() {
        let msg = CanMessage::new(std(0x123), &[1, 2, 3, 4]).unwrap();
        assert_eq!(msg.id(), 0x123);
        assert_eq!(msg.data(), &[1, 2, 3, 4]);
        assert_eq!(msg.len(), 4);
        assert_eq!(msg.dlc(), 4);
        assert!(msg.flags().contains(MessageFlags::STD));
        assert!(!msg.is_extended());
        assert!(!msg.is_fd());
        assert!(msg.timestamp().is_none());
    }

    #[test]
    fn new_with_extended_id_sets_ext_flag() {
        let msg = CanMessage::new(ext(0x1FFF_FFFF), &[0xAA, 0xBB]).unwrap();
        assert_eq!(msg.id(), 0x1FFF_FFFF);
        assert!(msg.is_extended());
        assert!(!msg.flags().contains(MessageFlags::STD));
        assert_eq!(msg.typed_id(), Some(Id::Extended(ext(0x1FFF_FFFF))));
    }

    #[test]
    fn new_fd_sets_fd_and_brs_flags() {
        let off = CanMessage::new_fd(std(0x100), &[1; 64], Brs::Off).unwrap();
        assert!(off.is_fd());
        assert!(!off.flags().contains(MessageFlags::BRS));
        let on = CanMessage::new_fd(std(0x100), &[2; 32], Brs::On).unwrap();
        assert!(on.flags().contains(MessageFlags::BRS));
    }

    #[test]
    fn fd_payload_is_padded_to_valid_length() {
        let cases: &[(usize, usize, u8)] = &[
            (9, 12, 9),
            (13, 16, 10),
            (17, 20, 11),
            (21, 24, 12),
            (25, 32, 13),
            (33, 48, 14),
            (49, 64, 15),
        ];
        for &(input, padded, dlc) in cases {
            let data: Vec<u8> = (0..input as u8).collect();
            let msg = CanMessage::new_fd(std(0x100), &data, Brs::Off).unwrap();
            assert_eq!(msg.len(), padded);
            assert_eq!(msg.dlc(), dlc);
            assert_eq!(&msg.data()[..input], &data[..]);
            assert!(msg.data()[input..].iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn dlc_and_len_round_trip() {
        for dlc in 0..=15u8 {
            assert_eq!(len_to_dlc(dlc_to_len(dlc, true)), dlc);
        }
        assert_eq!(dlc_to_len(15, false), 8);
        assert_eq!(dlc_to_len(0x1F, true), 64);
    }

    #[test]
    fn remote_frame_reports_requested_len() {
        let rtr = CanMessage::new_remote(std(0x10), 6).unwrap();
        assert!(rtr.is_remote());
        assert!(rtr.data().is_empty());
        assert_eq!(rtr.len(), 6);
        assert_eq!(rtr.dlc(), 6);
    }

    #[test]
    fn from_raw_dispatches_by_flag() {
        let err = CanMessage::from_raw(0, &[], 0, MessageFlags::ERROR_FRAME, None);
        assert!(err.is_error_frame());
        let rtr = CanMessage::from_raw(1, &[], 3, MessageFlags::STD | MessageFlags::RTR, None);
        assert!(rtr.is_remote());
        assert_eq!(rtr.len(), 3);
        let ts = Some(Duration::from_micros(42));
        let data = CanMessage::from_raw(2, &[1], 0, MessageFlags::STD, ts);
        assert!(matches!(data, CanMessage::Data(_)));
        assert_eq!(data.timestamp(), ts);
    }

    #[test]
    fn from_raw_error_takes_priority_over_rtr() {
        let flags = MessageFlags::ERROR_FRAME | MessageFlags::RTR;
        let m = CanMessage::from_raw(0, &[], 0, flags, None);
        assert!(m.is_error_frame());
        assert!(!m.is_remote());
    }

    #[test]
    fn oversized_payloads_are_rejected() {
        assert!(CanMessage::new(std(0x100), &[0; 9]).is_err());
        assert!(CanMessage::new_fd(std(0x100), &[0; 65], Brs::Off).is_err());
        assert!(CanMessage::new_remote(std(0x100), 9).is_err());
    }
}
