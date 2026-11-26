//! [`embedded_can`] integration.
//!
//! Provides:
//! - An [`embedded_can::Frame`] impl for [`CanMessage`].
//! - [`BusError`], a CAN protocol error decoded from a received error frame,
//!   keeping the raw [`MessageFlags`] error bits and timestamp alongside the
//!   categorised [`embedded_can::ErrorKind`].
//! - [`EmbeddedCanError`], distinguishing software/driver errors ([`Error`])
//!   from on-the-wire bus errors ([`BusError`]).
//! - [`EmbeddedCan`], an adapter implementing `embedded_can::nb::Can` and
//!   `embedded_can::blocking::Can` for any [`CanBus`].

use core::time::Duration;

use embedded_can::ErrorKind;

use crate::bus::CanBus;
use crate::error::Error;
use crate::frame::{CAN_MAX_LEN, CanMessage, MessageFlags};

/// A CAN bus protocol error decoded from a received error frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusError {
    /// Categorised error kind for `embedded_can::Error::kind()`.
    pub kind: ErrorKind,
    /// Raw error-frame flags.
    pub flags: MessageFlags,
    /// Timestamp of the error frame, if available.
    pub timestamp: Option<Duration>,
}

impl BusError {
    /// Decode an error frame's flags and timestamp.
    pub fn from_flags(flags: MessageFlags, timestamp: Option<Duration>) -> Self {
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

/// Error type bridging driver errors and CAN bus protocol errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbeddedCanError {
    /// Software/driver-layer error.
    #[error("CAN error: {0}")]
    Can(#[from] Error),
    /// CAN bus protocol error decoded from a received error frame.
    #[error("{0}")]
    Bus(BusError),
}

impl embedded_can::Error for EmbeddedCanError {
    fn kind(&self) -> ErrorKind {
        match self {
            EmbeddedCanError::Can(Error::Overrun) => ErrorKind::Overrun,
            EmbeddedCanError::Can(_) => ErrorKind::Other,
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
        if dlc > CAN_MAX_LEN {
            return None;
        }
        CanMessage::new_remote(id, dlc as u8).ok()
    }

    fn is_extended(&self) -> bool {
        CanMessage::is_extended(self)
    }

    fn is_remote_frame(&self) -> bool {
        CanMessage::is_remote(self)
    }

    fn id(&self) -> embedded_can::Id {
        self.typed_id().unwrap_or(if self.is_extended() {
            embedded_can::Id::Extended(embedded_can::ExtendedId::ZERO)
        } else {
            embedded_can::Id::Standard(embedded_can::StandardId::ZERO)
        })
    }

    fn dlc(&self) -> usize {
        // embedded-can models classic CAN, where DLC == payload length.
        CanMessage::len(self)
    }

    fn data(&self) -> &[u8] {
        CanMessage::data(self)
    }
}

fn received(msg: CanMessage) -> Result<CanMessage, EmbeddedCanError> {
    if msg.is_error_frame() {
        Err(EmbeddedCanError::Bus(BusError::from_flags(
            msg.flags(),
            msg.timestamp(),
        )))
    } else {
        Ok(msg)
    }
}

/// Non-blocking transmit for any [`CanBus`], per `embedded_can::nb::Can`.
pub fn nb_transmit<B: CanBus + ?Sized>(
    bus: &mut B,
    frame: &CanMessage,
) -> nb::Result<Option<CanMessage>, EmbeddedCanError> {
    match bus.send(frame, Some(Duration::ZERO)) {
        Ok(()) => Ok(None),
        Err(Error::TxQueueFull) | Err(Error::Timeout) => Err(nb::Error::WouldBlock),
        Err(e) => Err(nb::Error::Other(e.into())),
    }
}

/// Non-blocking receive for any [`CanBus`], per `embedded_can::nb::Can`.
///
/// Error frames are returned as [`EmbeddedCanError::Bus`].
pub fn nb_receive<B: CanBus + ?Sized>(bus: &mut B) -> nb::Result<CanMessage, EmbeddedCanError> {
    match bus.recv(Some(Duration::ZERO)) {
        Ok(Some(msg)) => received(msg).map_err(nb::Error::Other),
        Ok(None) => Err(nb::Error::WouldBlock),
        Err(e) => Err(nb::Error::Other(e.into())),
    }
}

/// Blocking transmit for any [`CanBus`], per `embedded_can::blocking::Can`.
pub fn blocking_transmit<B: CanBus + ?Sized>(
    bus: &mut B,
    frame: &CanMessage,
) -> Result<(), EmbeddedCanError> {
    Ok(bus.send(frame, None)?)
}

/// Blocking receive for any [`CanBus`], per `embedded_can::blocking::Can`.
///
/// Error frames are returned as [`EmbeddedCanError::Bus`].
pub fn blocking_receive<B: CanBus + ?Sized>(bus: &mut B) -> Result<CanMessage, EmbeddedCanError> {
    loop {
        if let Some(msg) = bus.recv(None)? {
            return received(msg);
        }
    }
}

/// Adapter implementing the `embedded-can` traits for any [`CanBus`].
///
/// ```
/// # fn wrap(bus: Box<dyn canbus_core::CanBus>) {
/// use canbus_core::EmbeddedCan;
/// let mut can = EmbeddedCan(bus);
/// // `can` now implements embedded_can::blocking::Can and nb::Can.
/// # let _ = &mut can;
/// # }
/// ```
#[derive(Debug)]
pub struct EmbeddedCan<B>(pub B);

impl<B: CanBus> embedded_can::nb::Can for EmbeddedCan<B> {
    type Frame = CanMessage;
    type Error = EmbeddedCanError;

    fn transmit(&mut self, frame: &CanMessage) -> nb::Result<Option<CanMessage>, Self::Error> {
        nb_transmit(&mut self.0, frame)
    }

    fn receive(&mut self) -> nb::Result<CanMessage, Self::Error> {
        nb_receive(&mut self.0)
    }
}

impl<B: CanBus> embedded_can::blocking::Can for EmbeddedCan<B> {
    type Frame = CanMessage;
    type Error = EmbeddedCanError;

    fn transmit(&mut self, frame: &CanMessage) -> Result<(), Self::Error> {
        blocking_transmit(&mut self.0, frame)
    }

    fn receive(&mut self) -> Result<CanMessage, Self::Error> {
        blocking_receive(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_recognises_each_bus_error() {
        let cases = [
            (MessageFlags::ERR_HW_OVERRUN, ErrorKind::Overrun),
            (MessageFlags::ERR_SW_OVERRUN, ErrorKind::Overrun),
            (MessageFlags::ERR_STUFF, ErrorKind::Stuff),
            (MessageFlags::ERR_FORM, ErrorKind::Form),
            (MessageFlags::ERR_CRC, ErrorKind::Crc),
            (MessageFlags::ERR_BIT0, ErrorKind::Bit),
            (MessageFlags::ERR_BIT1, ErrorKind::Bit),
            (MessageFlags::empty(), ErrorKind::Other),
        ];
        for (flags, kind) in cases {
            assert_eq!(decode_error_frame(flags), kind, "{flags:?}");
        }
    }

    #[test]
    fn decode_overrun_takes_precedence() {
        let flags = MessageFlags::ERR_SW_OVERRUN | MessageFlags::ERR_STUFF;
        assert_eq!(decode_error_frame(flags), ErrorKind::Overrun);
    }

    #[test]
    fn error_kinds() {
        use embedded_can::Error as _;
        assert_eq!(
            EmbeddedCanError::Can(Error::Hardware).kind(),
            ErrorKind::Other
        );
        assert_eq!(
            EmbeddedCanError::Can(Error::Overrun).kind(),
            ErrorKind::Overrun
        );
    }

    #[test]
    fn bus_error_preserves_flags_and_timestamp() {
        let flags = MessageFlags::ERROR_FRAME | MessageFlags::ERR_BIT0;
        let ts = Some(Duration::from_micros(12_345));
        let bus = BusError::from_flags(flags, ts);
        assert_eq!(bus.kind, ErrorKind::Bit);
        assert_eq!(bus.flags, flags);
        assert_eq!(bus.timestamp, ts);
    }
}
