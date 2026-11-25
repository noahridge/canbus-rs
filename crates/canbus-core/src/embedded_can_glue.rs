//! [`embedded_can`] integration.
//!
//! Provides:
//! - [`BusError`], a structured CAN bus protocol error decoded from a received
//!   error frame, preserving the raw `MessageFlags` error bits and hardware
//!   timestamp alongside the categorised [`embedded_can::ErrorKind`].
//! - [`EmbeddedCanError`], a wrapper that distinguishes software/driver errors
//!   ([`CanError`]) from on-the-wire bus errors ([`BusError`]).
//! - An [`embedded_can::Frame`] impl for [`CanMessage`].
//! - Generic blocking and non-blocking helpers backends can use to implement
//!   `embedded_can::nb::Can` and `embedded_can::blocking::Can`.

use core::time::Duration;

use embedded_can::ErrorKind;

use crate::channel::{CanRead, CanWrite};
use crate::error::CanError;
use crate::message::{CanMessage, MessageFlags, CAN_MAX_DLC};

/// A CAN bus protocol error decoded from a received error frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusError {
    /// Categorised error kind for `embedded_can::Error::kind()`.
    pub kind: ErrorKind,
    /// Raw error-frame flags.
    pub flags: MessageFlags,
    /// Hardware timestamp from the error frame, if available.
    pub timestamp: Option<u64>,
}

impl BusError {
    /// Build a [`BusError`] from the message flags and timestamp of an error frame.
    pub fn from_flags(flags: MessageFlags, timestamp: Option<u64>) -> Self {
        Self {
            kind: decode_error_frame(flags),
            flags,
            timestamp,
        }
    }
}

impl core::fmt::Display for BusError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CAN bus error: {:?} (flags={:?})", self.kind, self.flags)
    }
}

/// Error type bridging CAN driver errors and CAN bus protocol errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddedCanError {
    /// Software/driver-layer error.
    #[error("library error: {0}")]
    Lib(#[from] CanError),
    /// CAN bus protocol error decoded from a received error frame.
    #[error("{0}")]
    Bus(BusError),
}

impl embedded_can::Error for EmbeddedCanError {
    fn kind(&self) -> ErrorKind {
        match self {
            EmbeddedCanError::Lib(_) => ErrorKind::Other,
            EmbeddedCanError::Bus(b) => b.kind,
        }
    }
}

/// Decode the flags of an error frame into an [`embedded_can::ErrorKind`].
///
/// Overrun (HW or SW) takes precedence over protocol errors to surface
/// buffer-loss conditions explicitly.
pub fn decode_error_frame(flags: MessageFlags) -> ErrorKind {
    if flags.intersects(MessageFlags::ERR_HW_OVERRUN | MessageFlags::ERR_SW_OVERRUN) {
        ErrorKind::Overrun
    } else if flags.contains(MessageFlags::ERR_STUFF) {
        ErrorKind::Stuff
    } else if flags.contains(MessageFlags::ERR_FORM) {
        ErrorKind::Form
    } else if flags.contains(MessageFlags::ERR_CRC) {
        ErrorKind::Crc
    } else if flags.intersects(MessageFlags::ERR_BIT0 | MessageFlags::ERR_BIT1) {
        ErrorKind::Bit
    } else {
        ErrorKind::Other
    }
}

impl embedded_can::Frame for CanMessage {
    fn new(id: impl Into<embedded_can::Id>, data: &[u8]) -> Option<Self> {
        CanMessage::new(id, data).ok()
    }

    fn new_remote(id: impl Into<embedded_can::Id>, dlc: usize) -> Option<Self> {
        if dlc > CAN_MAX_DLC {
            return None;
        }
        CanMessage::new_remote(id, dlc as u8).ok()
    }

    fn is_extended(&self) -> bool {
        CanMessage::is_extended(self)
    }

    fn is_remote_frame(&self) -> bool {
        CanMessage::is_rtr(self)
    }

    fn id(&self) -> embedded_can::Id {
        let raw = CanMessage::id(self);
        if CanMessage::is_extended(self) {
            embedded_can::ExtendedId::new(raw)
                .map(embedded_can::Id::Extended)
                .unwrap_or(embedded_can::Id::Extended(embedded_can::ExtendedId::ZERO))
        } else {
            embedded_can::StandardId::new(raw as u16)
                .map(embedded_can::Id::Standard)
                .unwrap_or(embedded_can::Id::Standard(embedded_can::StandardId::ZERO))
        }
    }

    fn dlc(&self) -> usize {
        CanMessage::dlc(self) as usize
    }

    fn data(&self) -> &[u8] {
        CanMessage::data(self)
    }
}

/// Effectively-infinite timeout for blocking calls.
///
/// Most backend `*_wait` calls take a `c_ulong` ms; `u32::MAX` ms ≈ 49.7 days.
const FOREVER_MS: u64 = u32::MAX as u64;

/// Generic non-blocking transmit helper for backends.
pub fn nb_transmit<C: CanWrite>(
    chan: &C,
    frame: &CanMessage,
) -> nb::Result<Option<CanMessage>, EmbeddedCanError> {
    match chan.write(frame) {
        Ok(()) => Ok(None),
        Err(CanError::TxBufOverflow) => Err(nb::Error::WouldBlock),
        Err(e) => Err(nb::Error::Other(EmbeddedCanError::Lib(e))),
    }
}

/// Generic non-blocking receive helper for backends.
pub fn nb_receive<C: CanRead>(chan: &C) -> nb::Result<CanMessage, EmbeddedCanError> {
    match chan.read() {
        Ok(msg) if msg.is_error_frame() => Err(nb::Error::Other(EmbeddedCanError::Bus(
            BusError::from_flags(msg.flags(), msg.timestamp()),
        ))),
        Ok(msg) => Ok(msg),
        Err(CanError::NoMsg) => Err(nb::Error::WouldBlock),
        Err(e) => Err(nb::Error::Other(EmbeddedCanError::Lib(e))),
    }
}

/// Generic blocking transmit helper for backends.
pub fn blocking_transmit<C: CanWrite>(
    chan: &C,
    frame: &CanMessage,
) -> core::result::Result<(), EmbeddedCanError> {
    loop {
        match chan.write_wait(frame, Duration::from_millis(FOREVER_MS)) {
            Ok(()) => return Ok(()),
            Err(CanError::Timeout) => continue,
            Err(e) => return Err(EmbeddedCanError::Lib(e)),
        }
    }
}

/// Generic blocking receive helper for backends.
pub fn blocking_receive<C: CanRead>(
    chan: &C,
) -> core::result::Result<CanMessage, EmbeddedCanError> {
    loop {
        match chan.read_wait(Duration::from_millis(FOREVER_MS)) {
            Ok(msg) if msg.is_error_frame() => {
                return Err(EmbeddedCanError::Bus(BusError::from_flags(
                    msg.flags(),
                    msg.timestamp(),
                )));
            }
            Ok(msg) => return Ok(msg),
            Err(CanError::Timeout) => continue,
            Err(e) => return Err(EmbeddedCanError::Lib(e)),
        }
    }
}

/// Generate `embedded_can::nb::Can` and `embedded_can::blocking::Can` impls
/// for a backend channel by delegating to the generic helpers above.
#[macro_export]
macro_rules! impl_embedded_can {
    ($type:ty) => {
        impl $crate::embedded_can::nb::Can for $type {
            type Frame = $crate::message::CanMessage;
            type Error = $crate::embedded_can_glue::EmbeddedCanError;

            fn transmit(
                &mut self,
                frame: &Self::Frame,
            ) -> $crate::nb::Result<Option<Self::Frame>, Self::Error> {
                $crate::embedded_can_glue::nb_transmit(self, frame)
            }

            fn receive(&mut self) -> $crate::nb::Result<Self::Frame, Self::Error> {
                $crate::embedded_can_glue::nb_receive(self)
            }
        }

        impl $crate::embedded_can::blocking::Can for $type {
            type Frame = $crate::message::CanMessage;
            type Error = $crate::embedded_can_glue::EmbeddedCanError;

            fn transmit(&mut self, frame: &Self::Frame) -> core::result::Result<(), Self::Error> {
                $crate::embedded_can_glue::blocking_transmit(self, frame)
            }

            fn receive(&mut self) -> core::result::Result<Self::Frame, Self::Error> {
                $crate::embedded_can_glue::blocking_receive(self)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_recognises_each_bus_error() {
        assert_eq!(
            decode_error_frame(MessageFlags::ERR_HW_OVERRUN),
            ErrorKind::Overrun
        );
        assert_eq!(
            decode_error_frame(MessageFlags::ERR_SW_OVERRUN),
            ErrorKind::Overrun
        );
        assert_eq!(
            decode_error_frame(MessageFlags::ERR_STUFF),
            ErrorKind::Stuff
        );
        assert_eq!(decode_error_frame(MessageFlags::ERR_FORM), ErrorKind::Form);
        assert_eq!(decode_error_frame(MessageFlags::ERR_CRC), ErrorKind::Crc);
        assert_eq!(decode_error_frame(MessageFlags::ERR_BIT0), ErrorKind::Bit);
        assert_eq!(decode_error_frame(MessageFlags::ERR_BIT1), ErrorKind::Bit);
    }

    #[test]
    fn decode_overrun_takes_precedence() {
        let flags = MessageFlags::ERR_SW_OVERRUN | MessageFlags::ERR_STUFF;
        assert_eq!(decode_error_frame(flags), ErrorKind::Overrun);
    }

    #[test]
    fn lib_errors_map_to_other_kind() {
        let e = EmbeddedCanError::Lib(CanError::Hardware);
        assert_eq!(embedded_can::Error::kind(&e), ErrorKind::Other);
    }

    #[test]
    fn bus_error_preserves_flags_and_timestamp() {
        let flags = MessageFlags::ERROR_FRAME | MessageFlags::ERR_BIT0;
        let bus = BusError::from_flags(flags, Some(12_345));
        assert_eq!(bus.kind, ErrorKind::Bit);
        assert_eq!(bus.flags, flags);
        assert_eq!(bus.timestamp, Some(12_345));
    }
}
