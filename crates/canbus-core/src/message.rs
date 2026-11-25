//! CAN message types and flags, vendor-agnostic.
//!
//! [`MessageFlags`] uses bit positions chosen for this crate; each backend
//! translates to/from its native flag representation when crossing the FFI
//! boundary.

use embedded_can::Id;

use crate::error::{CanError, Result};

/// CAN FD Bit Rate Switch.
///
/// Controls whether the data phase of a CAN FD frame is transmitted at
/// the faster FD bitrate (`On`) or at the same bitrate as the
/// arbitration phase (`Off`).
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
    /// The bit values are crate-defined and do **not** match any specific
    /// vendor's wire representation. Backends translate when reading or
    /// writing FFI structures.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        /// Echo of a frame we transmitted (PCAN-style echo).
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

impl Default for MessageFlags {
    fn default() -> Self {
        MessageFlags::empty()
    }
}

/// Maximum data length for classic CAN.
pub const CAN_MAX_DLC: usize = 8;

/// Maximum data length for CAN FD.
pub const CANFD_MAX_DLC: usize = 64;

/// Valid CAN FD payload sizes above 8 bytes.
const CANFD_VALID_SIZES: [usize; 7] = [12, 16, 20, 24, 32, 48, 64];

/// Round a data length up to the nearest valid CAN FD DLC.
///
/// For lengths 0-8, returns the length as-is. For lengths 9-64, returns
/// the next valid FD size (12, 16, 20, 24, 32, 48, or 64).
pub fn fd_dlc_for_len(len: usize) -> usize {
    if len <= CAN_MAX_DLC {
        return len;
    }
    for &valid in &CANFD_VALID_SIZES {
        if len <= valid {
            return valid;
        }
    }
    CANFD_MAX_DLC
}

/// Maximum standard (11-bit) CAN identifier.
pub const CAN_STD_ID_MAX: u32 = 0x7FF;

/// Maximum extended (29-bit) CAN identifier.
pub const CAN_EXT_ID_MAX: u32 = 0x1FFF_FFFF;

/// A CAN data frame (classic or FD).
#[derive(Debug, Clone)]
pub struct DataFrame {
    pub(crate) id: u32,
    pub(crate) data: Vec<u8>,
    pub(crate) dlc: u8,
    pub(crate) flags: MessageFlags,
    pub(crate) timestamp: Option<u64>,
}

/// A CAN Remote Transmission Request frame.
#[derive(Debug, Clone)]
pub struct RemoteFrame {
    pub(crate) id: u32,
    pub(crate) dlc: u8,
    pub(crate) flags: MessageFlags,
    pub(crate) timestamp: Option<u64>,
}

/// A CAN error frame.
#[derive(Debug, Clone)]
pub struct ErrorFrame {
    pub(crate) id: u32,
    pub(crate) data: Vec<u8>,
    pub(crate) dlc: u8,
    pub(crate) flags: MessageFlags,
    pub(crate) timestamp: Option<u64>,
}

/// A CAN message (classic or FD).
///
/// Each variant enforces which fields and flag combinations are valid:
/// - `Data` — standard data frames (classic CAN and FD)
/// - `Remote` — Remote Transmission Request frames (no payload)
/// - `Error` — error frames from the controller
#[derive(Debug, Clone)]
pub enum CanMessage {
    /// A data frame (classic CAN or CAN FD).
    Data(DataFrame),
    /// A Remote Transmission Request frame.
    Remote(RemoteFrame),
    /// An error frame.
    Error(ErrorFrame),
}

/// Convert a typed `Id` into the raw `u32` and matching std/ext flag.
pub fn id_to_raw_and_flag(id: Id) -> (u32, MessageFlags) {
    match id {
        Id::Standard(s) => (u32::from(s.as_raw()), MessageFlags::STD),
        Id::Extended(e) => (e.as_raw(), MessageFlags::EXT),
    }
}

impl CanMessage {
    /// Create a classic CAN data frame.
    ///
    /// The `id` may be a [`StandardId`](embedded_can::StandardId), an
    /// [`ExtendedId`](embedded_can::ExtendedId), or an [`Id`]; the
    /// std/ext flag is taken from its variant.
    ///
    /// Returns `Err(CanError::Param)` if `data.len() > 8`.
    pub fn new(id: impl Into<Id>, data: &[u8]) -> Result<Self> {
        if data.len() > CAN_MAX_DLC {
            return Err(CanError::Param);
        }
        let (raw, flag) = id_to_raw_and_flag(id.into());
        Ok(CanMessage::Data(DataFrame {
            id: raw,
            data: data.to_vec(),
            dlc: data.len() as u8,
            flags: flag,
            timestamp: None,
        }))
    }

    /// Create a CAN FD data frame.
    ///
    /// The DLC is rounded up to the nearest valid CAN FD size
    /// (0-8, 12, 16, 20, 24, 32, 48, or 64) and the data payload
    /// is zero-padded to match. The std/ext flag is taken from
    /// the [`Id`] variant. Pass [`Brs::On`] to switch to the faster
    /// data-phase bitrate during the data field.
    ///
    /// Returns `Err(CanError::Param)` if `data.len() > 64`.
    pub fn new_fd(id: impl Into<Id>, data: &[u8], brs: Brs) -> Result<Self> {
        if data.len() > CANFD_MAX_DLC {
            return Err(CanError::Param);
        }
        let dlc = fd_dlc_for_len(data.len());
        let mut padded = vec![0u8; dlc];
        padded[..data.len()].copy_from_slice(data);

        let (raw, mut flags) = id_to_raw_and_flag(id.into());
        flags |= MessageFlags::FD;
        if brs == Brs::On {
            flags |= MessageFlags::BRS;
        }
        Ok(CanMessage::Data(DataFrame {
            id: raw,
            data: padded,
            dlc: dlc as u8,
            flags,
            timestamp: None,
        }))
    }

    /// Create a Remote Transmission Request frame.
    ///
    /// RTR frames carry no data payload — only the requested DLC.
    /// The std/ext flag is taken from the [`Id`] variant.
    ///
    /// Returns `Err(CanError::Param)` if `dlc > 8`.
    pub fn new_remote(id: impl Into<Id>, dlc: u8) -> Result<Self> {
        if dlc as usize > CAN_MAX_DLC {
            return Err(CanError::Param);
        }
        let (raw, flag) = id_to_raw_and_flag(id.into());
        Ok(CanMessage::Remote(RemoteFrame {
            id: raw,
            dlc,
            flags: flag | MessageFlags::RTR,
            timestamp: None,
        }))
    }

    /// Create a message from raw values (no validation), dispatching to
    /// the appropriate variant based on flags.
    ///
    /// - `ERROR_FRAME` → `CanMessage::Error`
    /// - `RTR` → `CanMessage::Remote` (data discarded)
    /// - otherwise → `CanMessage::Data`
    pub fn from_raw(id: u32, data: Vec<u8>, dlc: u8, flags: MessageFlags, timestamp: u64) -> Self {
        if flags.contains(MessageFlags::ERROR_FRAME) {
            CanMessage::Error(ErrorFrame {
                id,
                data,
                dlc,
                flags,
                timestamp: Some(timestamp),
            })
        } else if flags.contains(MessageFlags::RTR) {
            CanMessage::Remote(RemoteFrame {
                id,
                dlc,
                flags,
                timestamp: Some(timestamp),
            })
        } else {
            CanMessage::Data(DataFrame {
                id,
                data,
                dlc,
                flags,
                timestamp: Some(timestamp),
            })
        }
    }

    /// CAN identifier (11-bit or 29-bit).
    pub fn id(&self) -> u32 {
        match self {
            CanMessage::Data(f) => f.id,
            CanMessage::Remote(f) => f.id,
            CanMessage::Error(f) => f.id,
        }
    }

    /// Message data payload. Returns an empty slice for RTR frames.
    pub fn data(&self) -> &[u8] {
        match self {
            CanMessage::Data(f) => &f.data,
            CanMessage::Remote(_) => &[],
            CanMessage::Error(f) => &f.data,
        }
    }

    /// Data Length Code.
    pub fn dlc(&self) -> u8 {
        match self {
            CanMessage::Data(f) => f.dlc,
            CanMessage::Remote(f) => f.dlc,
            CanMessage::Error(f) => f.dlc,
        }
    }

    /// Message flags.
    pub fn flags(&self) -> MessageFlags {
        match self {
            CanMessage::Data(f) => f.flags,
            CanMessage::Remote(f) => f.flags,
            CanMessage::Error(f) => f.flags,
        }
    }

    /// Hardware timestamp in microseconds (populated on receive).
    pub fn timestamp(&self) -> Option<u64> {
        match self {
            CanMessage::Data(f) => f.timestamp,
            CanMessage::Remote(f) => f.timestamp,
            CanMessage::Error(f) => f.timestamp,
        }
    }

    /// Returns true if this is a CAN FD message.
    pub fn is_fd(&self) -> bool {
        self.flags().contains(MessageFlags::FD)
    }

    /// Returns true if this is an extended (29-bit) frame.
    pub fn is_extended(&self) -> bool {
        self.flags().contains(MessageFlags::EXT)
    }

    /// Returns true if this is a Remote Transmission Request.
    pub fn is_rtr(&self) -> bool {
        matches!(self, CanMessage::Remote(_))
    }

    /// Returns true if this is an error frame.
    pub fn is_error_frame(&self) -> bool {
        matches!(self, CanMessage::Error(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_can::{ExtendedId, StandardId};

    fn std(id: u16) -> StandardId {
        StandardId::new(id).expect("test id in range")
    }
    fn ext(id: u32) -> ExtendedId {
        ExtendedId::new(id).expect("test id in range")
    }

    #[test]
    fn new_sets_std_flag_and_correct_dlc() {
        let msg = CanMessage::new(std(0x123), &[1, 2, 3, 4]).unwrap();
        assert_eq!(msg.id(), 0x123);
        assert_eq!(msg.data(), &[1, 2, 3, 4]);
        assert_eq!(msg.dlc(), 4);
        assert!(msg.flags().contains(MessageFlags::STD));
        assert!(!msg.is_extended());
        assert!(!msg.is_fd());
        assert!(msg.timestamp().is_none());
    }

    #[test]
    fn new_with_extended_id_sets_ext_flag() {
        let msg = CanMessage::new(ext(0x1FFFFFFF), &[0xAA, 0xBB]).unwrap();
        assert_eq!(msg.id(), 0x1FFFFFFF);
        assert!(msg.is_extended());
        assert!(msg.flags().contains(MessageFlags::EXT));
        assert!(!msg.flags().contains(MessageFlags::STD));
    }

    #[test]
    fn new_fd_sets_fd_flag_and_brs_when_requested() {
        let msg_no_brs = CanMessage::new_fd(std(0x100), &[1; 64], Brs::Off).unwrap();
        assert!(msg_no_brs.is_fd());
        assert!(msg_no_brs.flags().contains(MessageFlags::FD));
        assert!(!msg_no_brs.flags().contains(MessageFlags::BRS));

        let msg_brs = CanMessage::new_fd(std(0x100), &[2; 32], Brs::On).unwrap();
        assert!(msg_brs.flags().contains(MessageFlags::BRS));
    }

    #[test]
    fn fd_dlc_rounds_up_to_valid_sizes() {
        let cases: &[(usize, usize)] = &[
            (9, 12),
            (13, 16),
            (17, 20),
            (21, 24),
            (25, 32),
            (33, 48),
            (49, 64),
        ];
        for &(input_len, expected_dlc) in cases {
            let data: Vec<u8> = (0..input_len as u8).collect();
            let msg = CanMessage::new_fd(std(0x100), &data, Brs::Off).unwrap();
            assert_eq!(msg.dlc() as usize, expected_dlc);
            assert_eq!(&msg.data()[..input_len], &data[..]);
            for &b in &msg.data()[input_len..] {
                assert_eq!(b, 0);
            }
        }
    }

    #[test]
    fn from_raw_dispatches_by_flag() {
        let err = CanMessage::from_raw(0, vec![], 0, MessageFlags::ERROR_FRAME, 0);
        assert!(err.is_error_frame());
        let rtr = CanMessage::from_raw(1, vec![], 0, MessageFlags::STD | MessageFlags::RTR, 0);
        assert!(rtr.is_rtr());
        let data = CanMessage::from_raw(2, vec![1], 1, MessageFlags::STD, 0);
        assert!(matches!(data, CanMessage::Data(_)));
    }

    #[test]
    fn from_raw_error_takes_priority_over_rtr() {
        let m = CanMessage::from_raw(
            0,
            vec![],
            0,
            MessageFlags::ERROR_FRAME | MessageFlags::RTR,
            0,
        );
        assert!(m.is_error_frame());
        assert!(!m.is_rtr());
    }

    #[test]
    fn new_rejects_oversized_data() {
        assert!(CanMessage::new(std(0x100), &[0; 9]).is_err());
        assert!(CanMessage::new_fd(std(0x100), &[0; 65], Brs::Off).is_err());
        assert!(CanMessage::new_remote(std(0x100), 9).is_err());
    }
}
