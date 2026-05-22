//! Kvaser-specific flag translation.
//!
//! The [`MessageFlags`](can_core::MessageFlags) type lives in `can-core`
//! with crate-defined bit positions. The CANLib FFI uses its own bit
//! positions (`canMSG_*`, `canFDMSG_*`, `canMSGERR_*`). These helpers
//! translate between the two when a frame crosses the FFI boundary.

use canlib_sys as sys;
use can_core::MessageFlags;

/// Translate a [`MessageFlags`] value into the raw CANLib flag bits.
pub(crate) fn flags_to_canlib(flags: MessageFlags) -> u32 {
    let mut out = 0u32;
    if flags.contains(MessageFlags::RTR) { out |= sys::canMSG_RTR; }
    if flags.contains(MessageFlags::STD) { out |= sys::canMSG_STD; }
    if flags.contains(MessageFlags::EXT) { out |= sys::canMSG_EXT; }
    if flags.contains(MessageFlags::ERROR_FRAME) { out |= sys::canMSG_ERROR_FRAME; }
    if flags.contains(MessageFlags::FD) { out |= sys::canFDMSG_FDF; }
    if flags.contains(MessageFlags::BRS) { out |= sys::canFDMSG_BRS; }
    if flags.contains(MessageFlags::ESI) { out |= sys::canFDMSG_ESI; }
    if flags.contains(MessageFlags::ERR_HW_OVERRUN) { out |= sys::canMSGERR_HW_OVERRUN; }
    if flags.contains(MessageFlags::ERR_SW_OVERRUN) { out |= sys::canMSGERR_SW_OVERRUN; }
    if flags.contains(MessageFlags::ERR_STUFF) { out |= sys::canMSGERR_STUFF; }
    if flags.contains(MessageFlags::ERR_FORM) { out |= sys::canMSGERR_FORM; }
    if flags.contains(MessageFlags::ERR_CRC) { out |= sys::canMSGERR_CRC; }
    if flags.contains(MessageFlags::ERR_BIT0) { out |= sys::canMSGERR_BIT0; }
    if flags.contains(MessageFlags::ERR_BIT1) { out |= sys::canMSGERR_BIT1; }
    out
}

/// Translate raw CANLib flag bits into a [`MessageFlags`] value.
pub(crate) fn flags_from_canlib(raw: u32) -> MessageFlags {
    let mut out = MessageFlags::empty();
    if raw & sys::canMSG_RTR != 0 { out |= MessageFlags::RTR; }
    if raw & sys::canMSG_STD != 0 { out |= MessageFlags::STD; }
    if raw & sys::canMSG_EXT != 0 { out |= MessageFlags::EXT; }
    if raw & sys::canMSG_ERROR_FRAME != 0 { out |= MessageFlags::ERROR_FRAME; }
    if raw & sys::canFDMSG_FDF != 0 { out |= MessageFlags::FD; }
    if raw & sys::canFDMSG_BRS != 0 { out |= MessageFlags::BRS; }
    if raw & sys::canFDMSG_ESI != 0 { out |= MessageFlags::ESI; }
    if raw & sys::canMSGERR_HW_OVERRUN != 0 { out |= MessageFlags::ERR_HW_OVERRUN; }
    if raw & sys::canMSGERR_SW_OVERRUN != 0 { out |= MessageFlags::ERR_SW_OVERRUN; }
    if raw & sys::canMSGERR_STUFF != 0 { out |= MessageFlags::ERR_STUFF; }
    if raw & sys::canMSGERR_FORM != 0 { out |= MessageFlags::ERR_FORM; }
    if raw & sys::canMSGERR_CRC != 0 { out |= MessageFlags::ERR_CRC; }
    if raw & sys::canMSGERR_BIT0 != 0 { out |= MessageFlags::ERR_BIT0; }
    if raw & sys::canMSGERR_BIT1 != 0 { out |= MessageFlags::ERR_BIT1; }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_round_trip_through_canlib() {
        let cases = [
            MessageFlags::STD,
            MessageFlags::EXT,
            MessageFlags::STD | MessageFlags::RTR,
            MessageFlags::STD | MessageFlags::FD | MessageFlags::BRS,
            MessageFlags::ERROR_FRAME | MessageFlags::ERR_BIT0,
            MessageFlags::EXT | MessageFlags::FD | MessageFlags::ESI,
        ];
        for f in cases {
            assert_eq!(flags_from_canlib(flags_to_canlib(f)), f);
        }
    }
}
