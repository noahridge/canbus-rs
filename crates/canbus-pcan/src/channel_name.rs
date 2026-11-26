//! PCAN channel names (`PCAN_USBBUS1`, …) ⇄ `TPCANHandle`.
//!
//! Names follow PEAK's header constants, which python-can uses too.

use crate::ffi as sys;

const FAMILIES: [(&str, [sys::TPCANHandle; 16]); 3] = [
    (
        "PCAN_USBBUS",
        [
            sys::PCAN_USBBUS1,
            sys::PCAN_USBBUS2,
            sys::PCAN_USBBUS3,
            sys::PCAN_USBBUS4,
            sys::PCAN_USBBUS5,
            sys::PCAN_USBBUS6,
            sys::PCAN_USBBUS7,
            sys::PCAN_USBBUS8,
            sys::PCAN_USBBUS9,
            sys::PCAN_USBBUS10,
            sys::PCAN_USBBUS11,
            sys::PCAN_USBBUS12,
            sys::PCAN_USBBUS13,
            sys::PCAN_USBBUS14,
            sys::PCAN_USBBUS15,
            sys::PCAN_USBBUS16,
        ],
    ),
    (
        "PCAN_PCIBUS",
        [
            sys::PCAN_PCIBUS1,
            sys::PCAN_PCIBUS2,
            sys::PCAN_PCIBUS3,
            sys::PCAN_PCIBUS4,
            sys::PCAN_PCIBUS5,
            sys::PCAN_PCIBUS6,
            sys::PCAN_PCIBUS7,
            sys::PCAN_PCIBUS8,
            sys::PCAN_PCIBUS9,
            sys::PCAN_PCIBUS10,
            sys::PCAN_PCIBUS11,
            sys::PCAN_PCIBUS12,
            sys::PCAN_PCIBUS13,
            sys::PCAN_PCIBUS14,
            sys::PCAN_PCIBUS15,
            sys::PCAN_PCIBUS16,
        ],
    ),
    (
        "PCAN_LANBUS",
        [
            sys::PCAN_LANBUS1,
            sys::PCAN_LANBUS2,
            sys::PCAN_LANBUS3,
            sys::PCAN_LANBUS4,
            sys::PCAN_LANBUS5,
            sys::PCAN_LANBUS6,
            sys::PCAN_LANBUS7,
            sys::PCAN_LANBUS8,
            sys::PCAN_LANBUS9,
            sys::PCAN_LANBUS10,
            sys::PCAN_LANBUS11,
            sys::PCAN_LANBUS12,
            sys::PCAN_LANBUS13,
            sys::PCAN_LANBUS14,
            sys::PCAN_LANBUS15,
            sys::PCAN_LANBUS16,
        ],
    ),
];

/// Parse a channel name such as `PCAN_USBBUS1` (case-insensitive) or a raw
/// handle such as `0x51`.
pub(crate) fn parse(name: &str) -> Option<sys::TPCANHandle> {
    let name = name.trim().to_ascii_uppercase();
    if let Some(hex) = name.strip_prefix("0X") {
        return sys::TPCANHandle::from_str_radix(hex, 16).ok();
    }
    FAMILIES.iter().find_map(|(prefix, handles)| {
        let n: usize = name.strip_prefix(prefix)?.parse().ok()?;
        handles.get(n.checked_sub(1)?).copied()
    })
}

/// The channel name for a handle, falling back to hex for handles without
/// a named constant.
pub(crate) fn name(handle: sys::TPCANHandle) -> String {
    FAMILIES
        .iter()
        .find_map(|(prefix, handles)| {
            let i = handles.iter().position(|&h| h == handle)?;
            Some(format!("{prefix}{}", i + 1))
        })
        .unwrap_or_else(|| format!("0x{handle:X}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_header_names() {
        assert_eq!(parse("PCAN_USBBUS1"), Some(0x51));
        assert_eq!(parse("pcan_usbbus8"), Some(0x58));
        assert_eq!(parse("PCAN_USBBUS9"), Some(0x509));
        assert_eq!(parse("PCAN_USBBUS16"), Some(0x510));
        assert_eq!(parse("PCAN_PCIBUS1"), Some(0x41));
        assert_eq!(parse("PCAN_LANBUS3"), Some(0x803));
        assert_eq!(parse("0x51"), Some(0x51));
    }

    #[test]
    fn rejects_out_of_range_and_garbage() {
        assert_eq!(parse("PCAN_USBBUS0"), None);
        assert_eq!(parse("PCAN_USBBUS17"), None);
        assert_eq!(parse("usb1"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn names_round_trip() {
        for (_, handles) in FAMILIES {
            for h in handles {
                assert_eq!(parse(&name(h)), Some(h));
            }
        }
        assert_eq!(name(0x99), "0x99");
    }
}
