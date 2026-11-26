//! Receive filters.

use crate::frame::{CanMessage, CAN_EXT_ID_MAX, CAN_STD_ID_MAX};

/// A receive filter. A bus with several filters accepts a frame if **any**
/// filter matches; an empty filter list accepts everything.
///
/// Filters apply to the frame's identifier and format (standard or
/// extended). Error frames always pass, so bus errors are never hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Accept IDs where `id & mask == code & mask`.
    CodeMask {
        /// Bits the ID must have where `mask` is set.
        code: u32,
        /// Which ID bits are compared.
        mask: u32,
        /// Apply to extended (29-bit) rather than standard frames.
        extended: bool,
    },
    /// Accept IDs in the inclusive range `from..=to`.
    Range {
        /// Lowest accepted ID.
        from: u32,
        /// Highest accepted ID.
        to: u32,
        /// Apply to extended (29-bit) rather than standard frames.
        extended: bool,
    },
}

impl Filter {
    /// A filter accepting exactly one standard ID.
    pub fn standard(id: u16) -> Self {
        Filter::CodeMask {
            code: u32::from(id),
            mask: CAN_STD_ID_MAX,
            extended: false,
        }
    }

    /// A filter accepting exactly one extended ID.
    pub fn extended(id: u32) -> Self {
        Filter::CodeMask {
            code: id,
            mask: CAN_EXT_ID_MAX,
            extended: true,
        }
    }

    /// True if this filter accepts `msg`.
    pub fn matches(&self, msg: &CanMessage) -> bool {
        if msg.is_error_frame() {
            return true;
        }
        let id = msg.id();
        match *self {
            Filter::CodeMask {
                code,
                mask,
                extended,
            } => msg.is_extended() == extended && id & mask == code & mask,
            Filter::Range { from, to, extended } => {
                msg.is_extended() == extended && (from..=to).contains(&id)
            }
        }
    }

    /// True if any filter in `filters` accepts `msg` (or `filters` is empty).
    pub fn any_matches(filters: &[Filter], msg: &CanMessage) -> bool {
        filters.is_empty() || filters.iter().any(|f| f.matches(msg))
    }
}

/// How a backend applied a set of filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterSupport {
    /// The hardware/driver enforces the filters exactly; every received
    /// frame already matches.
    Hardware,
    /// The backend could not enforce the filters exactly. It may have
    /// applied a looser hardware filter (or none), so the caller must check
    /// received frames with [`Filter::any_matches`]. The `canbus::Bus`
    /// facade does this automatically.
    Software,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{CanMessage, MessageFlags};
    use embedded_can::{ExtendedId, StandardId};

    fn std_msg(id: u16) -> CanMessage {
        CanMessage::new(StandardId::new(id).unwrap(), &[]).unwrap()
    }
    fn ext_msg(id: u32) -> CanMessage {
        CanMessage::new(ExtendedId::new(id).unwrap(), &[]).unwrap()
    }

    #[test]
    fn code_mask_matches_masked_bits() {
        let f = Filter::CodeMask {
            code: 0x100,
            mask: 0x7F0,
            extended: false,
        };
        assert!(f.matches(&std_msg(0x100)));
        assert!(f.matches(&std_msg(0x10F)));
        assert!(!f.matches(&std_msg(0x110)));
        assert!(!f.matches(&ext_msg(0x100)), "format must match");
    }

    #[test]
    fn range_is_inclusive() {
        let f = Filter::Range {
            from: 0x10,
            to: 0x20,
            extended: true,
        };
        assert!(f.matches(&ext_msg(0x10)));
        assert!(f.matches(&ext_msg(0x20)));
        assert!(!f.matches(&ext_msg(0x21)));
        assert!(!f.matches(&std_msg(0x15)));
    }

    #[test]
    fn exact_helpers() {
        assert!(Filter::standard(0x7FF).matches(&std_msg(0x7FF)));
        assert!(!Filter::standard(0x7FF).matches(&std_msg(0x7FE)));
        assert!(Filter::extended(0x1234_5678).matches(&ext_msg(0x1234_5678)));
    }

    #[test]
    fn empty_list_accepts_all_and_errors_always_pass() {
        assert!(Filter::any_matches(&[], &std_msg(1)));
        let err = CanMessage::from_raw(0, &[], 0, MessageFlags::ERROR_FRAME, None);
        assert!(Filter::any_matches(&[Filter::standard(5)], &err));
        assert!(!Filter::any_matches(&[Filter::standard(5)], &std_msg(6)));
    }
}
