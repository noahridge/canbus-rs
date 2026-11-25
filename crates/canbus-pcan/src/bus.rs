//! Channel addressing for PCAN-Basic.

use crate::ffi as sys;
use canbus_core::CanError;

/// A PCAN channel identifier.
///
/// PCAN addresses channels by hardware type + 1-based index (e.g.
/// `USBBUS3` = `0x53`). [`PcanBus::to_handle`] converts to the raw
/// `TPCANHandle` for FFI calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcanBus {
    /// PCAN-USB channel 1..=16.
    Usb(u8),
    /// PCAN-PCI channel 1..=16.
    Pci(u8),
    /// PCAN-LAN channel 1..=16.
    Lan(u8),
}

impl PcanBus {
    /// Convert to the raw `TPCANHandle`.
    ///
    /// Returns `Err(CanError::Param)` if the index is out of range
    /// (must be 1..=16).
    pub fn to_handle(self) -> Result<sys::TPCANHandle, CanError> {
        match self {
            PcanBus::Usb(i) => usb_handle(i),
            PcanBus::Pci(i) => pci_handle(i),
            PcanBus::Lan(i) => lan_handle(i),
        }
    }
}

fn usb_handle(i: u8) -> Result<sys::TPCANHandle, CanError> {
    match i {
        1 => Ok(sys::PCAN_USBBUS1),
        2 => Ok(sys::PCAN_USBBUS2),
        3 => Ok(sys::PCAN_USBBUS3),
        4 => Ok(sys::PCAN_USBBUS4),
        5 => Ok(sys::PCAN_USBBUS5),
        6 => Ok(sys::PCAN_USBBUS6),
        7 => Ok(sys::PCAN_USBBUS7),
        8 => Ok(sys::PCAN_USBBUS8),
        9 => Ok(sys::PCAN_USBBUS9),
        10 => Ok(sys::PCAN_USBBUS10),
        11 => Ok(sys::PCAN_USBBUS11),
        12 => Ok(sys::PCAN_USBBUS12),
        13 => Ok(sys::PCAN_USBBUS13),
        14 => Ok(sys::PCAN_USBBUS14),
        15 => Ok(sys::PCAN_USBBUS15),
        16 => Ok(sys::PCAN_USBBUS16),
        _ => Err(CanError::Param),
    }
}

fn pci_handle(i: u8) -> Result<sys::TPCANHandle, CanError> {
    match i {
        1 => Ok(sys::PCAN_PCIBUS1),
        2 => Ok(sys::PCAN_PCIBUS2),
        3 => Ok(sys::PCAN_PCIBUS3),
        4 => Ok(sys::PCAN_PCIBUS4),
        5 => Ok(sys::PCAN_PCIBUS5),
        6 => Ok(sys::PCAN_PCIBUS6),
        7 => Ok(sys::PCAN_PCIBUS7),
        8 => Ok(sys::PCAN_PCIBUS8),
        9 => Ok(sys::PCAN_PCIBUS9),
        10 => Ok(sys::PCAN_PCIBUS10),
        11 => Ok(sys::PCAN_PCIBUS11),
        12 => Ok(sys::PCAN_PCIBUS12),
        13 => Ok(sys::PCAN_PCIBUS13),
        14 => Ok(sys::PCAN_PCIBUS14),
        15 => Ok(sys::PCAN_PCIBUS15),
        16 => Ok(sys::PCAN_PCIBUS16),
        _ => Err(CanError::Param),
    }
}

fn lan_handle(i: u8) -> Result<sys::TPCANHandle, CanError> {
    match i {
        1 => Ok(sys::PCAN_LANBUS1),
        2 => Ok(sys::PCAN_LANBUS2),
        3 => Ok(sys::PCAN_LANBUS3),
        4 => Ok(sys::PCAN_LANBUS4),
        5 => Ok(sys::PCAN_LANBUS5),
        6 => Ok(sys::PCAN_LANBUS6),
        7 => Ok(sys::PCAN_LANBUS7),
        8 => Ok(sys::PCAN_LANBUS8),
        9 => Ok(sys::PCAN_LANBUS9),
        10 => Ok(sys::PCAN_LANBUS10),
        11 => Ok(sys::PCAN_LANBUS11),
        12 => Ok(sys::PCAN_LANBUS12),
        13 => Ok(sys::PCAN_LANBUS13),
        14 => Ok(sys::PCAN_LANBUS14),
        15 => Ok(sys::PCAN_LANBUS15),
        16 => Ok(sys::PCAN_LANBUS16),
        _ => Err(CanError::Param),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_handles_match_header() {
        assert_eq!(PcanBus::Usb(1).to_handle().unwrap(), 0x51);
        assert_eq!(PcanBus::Usb(8).to_handle().unwrap(), 0x58);
        assert_eq!(PcanBus::Usb(9).to_handle().unwrap(), 0x509);
        assert_eq!(PcanBus::Usb(16).to_handle().unwrap(), 0x510);
    }

    #[test]
    fn pci_handles_match_header() {
        assert_eq!(PcanBus::Pci(1).to_handle().unwrap(), 0x41);
        assert_eq!(PcanBus::Pci(9).to_handle().unwrap(), 0x409);
    }

    #[test]
    fn out_of_range_returns_param() {
        assert_eq!(PcanBus::Usb(0).to_handle(), Err(CanError::Param));
        assert_eq!(PcanBus::Usb(17).to_handle(), Err(CanError::Param));
    }
}
