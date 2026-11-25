use core::time::Duration;
use std::cell::Cell;
use std::marker::PhantomData;

use crate::ffi as sys;
use canbus_core::{
    impl_can_channel, impl_embedded_can, Bitrate, BusParams, BusStatistics, BusStatus, CanError,
    CanMessage, ErrorCounters, Filter, Result, CANFD_MAX_DLC,
};
use embedded_can::Id;

use crate::bitrate::{bitrate_to_btr, PcanFdBitrate};
use crate::bus::PcanBus;
use crate::error::{check_status, lib};
use crate::event::{self, wait_for_event, EventHandle, WaitOutcome};
use crate::flags::{fd_dlc_to_len, fd_len_to_dlc, flags_to_pcan, read_flags};

/// What bitrate has been configured on a [`PcanChannel`], if any.
///
/// PCAN's `CAN_Initialize` requires the bitrate up-front, so the safe
/// wrapper stashes it and applies it at `bus_on` time. CAN FD uses a
/// different init function, so we track which one.
#[derive(Debug, Clone)]
enum BitrateConfig {
    Classic(sys::TPCANBaudrate),
    Fd(PcanFdBitrate),
}

/// A handle to a PCAN-Basic channel.
///
/// PCAN's lifecycle differs from Kvaser's: `CAN_Initialize` combines
/// open + set-bitrate + bus-on, and `CAN_Uninitialize` is the off-bus
/// equivalent. To match the [`CanChannel`](canbus_core::CanChannel) trait
/// shape, this type defers initialisation until [`Self::bus_on`] is
/// called. Re-calling [`Self::set_bitrate`] after `bus_on` will
/// uninitialise and reinitialise the channel.
///
/// # Thread safety
///
/// `PcanChannel` is `!Send` and `!Sync` for parity with the Kvaser
/// backend, even though PCAN-Basic handles are thread-safe.
pub struct PcanChannel {
    handle: sys::TPCANHandle,
    bitrate: Cell<Option<BitrateConfig>>,
    on_bus: bool,
    event: Cell<Option<EventHandle>>,
    _not_send: PhantomData<*const ()>,
}

impl PcanChannel {
    /// Open a PCAN channel addressed by [`PcanBus`].
    ///
    /// Stores the handle and prepares for configuration. The channel
    /// is not initialised on the bus until [`Self::bus_on`] is called
    /// (which requires a bitrate to have been set first).
    pub fn open(bus: PcanBus) -> Result<Self> {
        let handle = bus.to_handle()?;
        Ok(Self {
            handle,
            bitrate: Cell::new(None),
            on_bus: false,
            event: Cell::new(None),
            _not_send: PhantomData,
        })
    }

    /// Raw PCAN channel handle.
    pub fn raw_handle(&self) -> sys::TPCANHandle {
        self.handle
    }

    // ---- Bus control ----

    pub fn bus_on(&mut self) -> Result<()> {
        let l = lib()?;
        match self.bitrate.take() {
            Some(BitrateConfig::Classic(btr)) => {
                self.bitrate.set(Some(BitrateConfig::Classic(btr)));
                check_status(unsafe { (l.CAN_Initialize)(self.handle, btr, 0, 0, 0) })?;
            }
            Some(BitrateConfig::Fd(fd)) => {
                let ptr = fd.0.as_ptr() as *mut std::os::raw::c_char;
                check_status(unsafe { (l.CAN_InitializeFD)(self.handle, ptr) })?;
                self.bitrate.set(Some(BitrateConfig::Fd(fd)));
            }
            None => return Err(CanError::Param),
        }
        self.on_bus = true;
        self.acquire_event(l);
        Ok(())
    }

    pub fn bus_off(&mut self) -> Result<()> {
        if !self.on_bus {
            return Ok(());
        }
        let l = lib()?;
        // Deregister and free the receive event before uninitialising, so
        // it is released even if CAN_Uninitialize reports an error.
        self.release_event(l);
        check_status(unsafe { (l.CAN_Uninitialize)(self.handle) })?;
        self.on_bus = false;
        Ok(())
    }

    /// Cache the receive-event handle for use by `read_wait`. If the
    /// platform/driver can't provide one, leaves `self.event` as `None`
    /// so the caller falls back to polling.
    fn acquire_event(&self, l: &sys::PcanLib) {
        if let Some(h) = event::acquire(l, self.handle) {
            self.event.set(Some(h));
        }
    }

    /// Deregister and free any cached receive-event handle.
    fn release_event(&self, l: &sys::PcanLib) {
        if let Some(ev) = self.event.take() {
            event::release(l, self.handle, ev);
        }
    }

    pub fn reset_bus(&mut self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.CAN_Reset)(self.handle) })
    }

    pub fn is_on_bus(&self) -> bool {
        self.on_bus
    }

    pub fn set_bitrate(&self, bitrate: Bitrate) -> Result<()> {
        let btr = bitrate_to_btr(bitrate)?;
        if self.on_bus {
            // Re-initialise with new bitrate. CAN_Uninitialize tears down the
            // driver-side channel state, including any registered receive
            // event — release ours first, then re-acquire against the fresh
            // channel so read_wait keeps getting notifications.
            let l = lib()?;
            self.release_event(l);
            check_status(unsafe { (l.CAN_Uninitialize)(self.handle) })?;
            check_status(unsafe { (l.CAN_Initialize)(self.handle, btr, 0, 0, 0) })?;
            self.acquire_event(l);
        }
        self.bitrate.set(Some(BitrateConfig::Classic(btr)));
        Ok(())
    }

    /// PCAN doesn't expose freq/tseg-style classic timing through
    /// `CAN_Initialize` — only BTR0/BTR1 codes via the predefined
    /// `Bitrate` enum. For custom timing, fall back to the FD bitrate
    /// string form (which works for classic CAN too).
    pub fn set_bus_params(&self, _params: &BusParams) -> Result<()> {
        Err(CanError::NotImplemented)
    }

    /// Set a CAN FD bitrate. The channel will be initialised in FD mode
    /// the next time [`Self::bus_on`] is called.
    pub fn set_fd_bitrate(&self, bitrate: PcanFdBitrate) -> Result<()> {
        if self.on_bus {
            // See set_bitrate for why the receive event is cycled here.
            let l = lib()?;
            self.release_event(l);
            check_status(unsafe { (l.CAN_Uninitialize)(self.handle) })?;
            let ptr = bitrate.0.as_ptr() as *mut std::os::raw::c_char;
            check_status(unsafe { (l.CAN_InitializeFD)(self.handle, ptr) })?;
            self.acquire_event(l);
        }
        self.bitrate.set(Some(BitrateConfig::Fd(bitrate)));
        Ok(())
    }

    fn is_fd_mode(&self) -> bool {
        // Peek without disturbing.
        let cell = self.bitrate.take();
        let is_fd = matches!(cell, Some(BitrateConfig::Fd(_)));
        self.bitrate.set(cell);
        is_fd
    }

    // ---- Write ----

    pub fn write(&self, msg: &CanMessage) -> Result<()> {
        let l = lib()?;
        if self.is_fd_mode() || msg.is_fd() {
            let src = msg.data();
            let len = src.len().min(CANFD_MAX_DLC);
            let mut m = sys::TPCANMsgFD {
                ID: msg.id(),
                MSGTYPE: flags_to_pcan(msg.flags()),
                DLC: fd_len_to_dlc(len),
                ..Default::default()
            };
            m.DATA[..len].copy_from_slice(&src[..len]);
            check_status(unsafe { (l.CAN_WriteFD)(self.handle, &mut m) })
        } else {
            let src = msg.data();
            let len = src.len().min(8);
            let mut m = sys::TPCANMsg {
                ID: msg.id(),
                MSGTYPE: flags_to_pcan(msg.flags()),
                LEN: len as u8,
                ..Default::default()
            };
            m.DATA[..len].copy_from_slice(&src[..len]);
            check_status(unsafe { (l.CAN_Write)(self.handle, &mut m) })
        }
    }

    pub fn write_wait(&self, msg: &CanMessage, timeout: Duration) -> Result<()> {
        // PCAN has no native blocking write. Poll until the tx queue
        // accepts the frame or the timeout expires.
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.write(msg) {
                Ok(()) => return Ok(()),
                Err(CanError::TxBufOverflow) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(CanError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn write_sync(&self, _timeout: Duration) -> Result<()> {
        // PCAN doesn't expose an equivalent of canWriteSync (waiting for
        // the TX queue to drain). Return NotImplemented rather than
        // silently succeeding.
        Err(CanError::NotImplemented)
    }

    // ---- Read ----

    pub fn read(&self) -> Result<CanMessage> {
        let l = lib()?;
        if self.is_fd_mode() {
            let mut m = sys::TPCANMsgFD::default();
            let mut ts: sys::TPCANTimestampFD = 0;
            check_status(unsafe { (l.CAN_ReadFD)(self.handle, &mut m, &mut ts) })?;
            // `fd_dlc_to_len` is bounded to 0..=64 and DATA is [u8; 64].
            let len = fd_dlc_to_len(m.DLC);
            let data = m.DATA[..len].to_vec();
            let flags = read_flags(m.MSGTYPE, &data);
            // `CanMessage::dlc` holds the payload length in bytes (as the
            // Kvaser backend does), not PCAN's 4-bit DLC code.
            Ok(CanMessage::from_raw(m.ID, data, len as u8, flags, ts))
        } else {
            let mut m = sys::TPCANMsg::default();
            let mut ts = sys::TPCANTimestamp::default();
            check_status(unsafe { (l.CAN_Read)(self.handle, &mut m, &mut ts) })?;
            // Clamp to the classic-frame buffer size: TPCANMsg::DATA is
            // [u8; 8], so a misreported LEN > 8 would otherwise panic.
            let len = (m.LEN as usize).min(8);
            let data = m.DATA[..len].to_vec();
            let flags = read_flags(m.MSGTYPE, &data);
            Ok(CanMessage::from_raw(
                m.ID,
                data,
                m.LEN,
                flags,
                ts.total_micros(),
            ))
        }
    }

    pub fn read_wait(&self, timeout: Duration) -> Result<CanMessage> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            // Fast path / drain: read whatever is already queued.
            match self.read() {
                Ok(msg) => return Ok(msg),
                Err(CanError::NoMsg) => {}
                Err(e) => return Err(e),
            }

            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(CanError::Timeout);
            }

            // Block on the receive event when available, otherwise poll. A
            // spurious wake-up (signalled but nothing queued) just loops and
            // re-reads until the deadline, so read_wait never returns NoMsg.
            if let Some(h) = self.event.get() {
                match wait_for_event(h, remaining) {
                    WaitOutcome::Signalled => continue,
                    WaitOutcome::Timeout => return Err(CanError::Timeout),
                    WaitOutcome::Failed => return Err(CanError::Driver),
                }
            } else {
                std::thread::sleep(Duration::from_millis(1).min(remaining));
            }
        }
    }

    pub fn read_specific(&self, _id: Id) -> Result<CanMessage> {
        // PCAN doesn't have a read_specific call; would need to read
        // and re-queue. Return NotImplemented to keep behaviour explicit.
        Err(CanError::NotImplemented)
    }

    pub fn read_specific_skip(&self, _id: Id) -> Result<CanMessage> {
        Err(CanError::NotImplemented)
    }

    // ---- Diagnostics ----

    pub fn set_acceptance_filter(&self, filter: &Filter) -> Result<()> {
        let l = lib()?;
        let (from, to, extended) = match *filter {
            Filter::Range { from, to, extended } => (from, to, extended),
            Filter::CodeMask {
                code,
                mask,
                extended,
            } => {
                let (from, to) = codemask_to_range(code, mask, extended)?;
                (from, to, extended)
            }
        };
        let mode = if extended {
            sys::PCAN_MODE_EXTENDED
        } else {
            sys::PCAN_MODE_STANDARD
        };
        check_status(unsafe { (l.CAN_FilterMessages)(self.handle, from, to, mode) })
    }

    pub fn read_status(&self) -> Result<BusStatus> {
        let l = lib()?;
        let raw = unsafe { (l.CAN_GetStatus)(self.handle) };
        Ok(pcan_status_to_bus_status(raw))
    }

    pub fn read_error_counters(&self) -> Result<ErrorCounters> {
        // PCAN-Basic does not expose TX/RX/overrun error counters directly.
        Err(CanError::NotImplemented)
    }

    /// Flush the receive queue.
    ///
    /// PCAN-Basic only exposes `CAN_Reset`, which clears **both** queues, so
    /// this call also discards anything pending in the transmit queue. There
    /// is no rx-only flush primitive in PCAN-Basic.
    pub fn flush_rx(&self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.CAN_Reset)(self.handle) })
    }

    /// Flush the transmit queue — not supported by PCAN-Basic.
    ///
    /// PCAN's only flush primitive (`CAN_Reset`) clears both queues, so a
    /// tx-only flush would silently drop received frames. Returning
    /// [`CanError::NotImplemented`] forces the caller to opt in by calling
    /// [`Self::flush_rx`] explicitly when they want to clear both queues.
    pub fn flush_tx(&self) -> Result<()> {
        Err(CanError::NotImplemented)
    }

    /// Request bus statistics — not supported by PCAN-Basic.
    pub fn request_bus_statistics(&self) -> Result<()> {
        Err(CanError::NotImplemented)
    }

    /// Get bus statistics — not supported by PCAN-Basic.
    pub fn get_bus_statistics(&self) -> Result<BusStatistics> {
        Err(CanError::NotImplemented)
    }
}

impl Drop for PcanChannel {
    fn drop(&mut self) {
        if self.on_bus {
            if let Ok(l) = lib() {
                self.release_event(l);
                unsafe {
                    (l.CAN_Uninitialize)(self.handle);
                }
            }
        }
    }
}

/// Translate a `TPCANStatus` from `CAN_GetStatus` into a [`BusStatus`].
///
/// `CAN_GetStatus` is documented as reporting only bus-state conditions, so
/// `PCAN_ERROR_OK` and any value with no recognised bus-state bit both map
/// to `ERROR_ACTIVE`. Extras like `PCAN_ERROR_QRCVEMPTY` are not bus errors
/// and must not surface from this call as `CanError::NoMsg`.
fn pcan_status_to_bus_status(raw: sys::TPCANStatus) -> BusStatus {
    if raw == sys::PCAN_ERROR_OK {
        return BusStatus::ERROR_ACTIVE;
    }
    let mut out = BusStatus::empty();
    if raw & sys::PCAN_ERROR_BUSOFF != 0 {
        out |= BusStatus::BUS_OFF;
    }
    if raw & sys::PCAN_ERROR_BUSPASSIVE != 0 {
        out |= BusStatus::ERROR_PASSIVE;
    }
    if raw & sys::PCAN_ERROR_BUSWARNING != 0 {
        out |= BusStatus::ERROR_WARNING;
    }
    if raw & (sys::PCAN_ERROR_OVERRUN | sys::PCAN_ERROR_QOVERRUN) != 0 {
        out |= BusStatus::OVERRUN;
    }
    if out.is_empty() {
        BusStatus::ERROR_ACTIVE
    } else {
        out
    }
}

/// Translate a Kvaser-style code/mask filter into an inclusive PCAN range.
///
/// Works only when the don't-care bits (`!mask`) within the ID width form a
/// single contiguous low-order block — otherwise the filter is not a single
/// range and we return [`CanError::NotImplemented`]. The `& id_bits` is
/// essential: without it the high bits of `!mask` (always set for an 11- or
/// 29-bit filter) defeat the contiguity check and every filter is rejected.
fn codemask_to_range(code: u32, mask: u32, extended: bool) -> Result<(u32, u32)> {
    let id_bits: u32 = if extended { 0x1FFF_FFFF } else { 0x7FF };
    let inv = !mask & id_bits;
    // `inv` must be of the form (1<<n)-1 — i.e. only the low n bits.
    if inv & inv.wrapping_add(1) != 0 {
        return Err(CanError::NotImplemented);
    }
    let from = code & mask & id_bits;
    let to = (code | inv) & id_bits;
    Ok((from, to))
}

impl_can_channel!(PcanChannel);
impl_embedded_can!(PcanChannel);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fd_mode_inspection_does_not_consume_bitrate() {
        let ch = PcanChannel {
            handle: 0x51,
            bitrate: Cell::new(Some(BitrateConfig::Classic(sys::PCAN_BAUD_500K))),
            on_bus: false,
            event: Cell::new(None),
            _not_send: PhantomData,
        };
        assert!(!ch.is_fd_mode());
        // Should still be set after the check.
        let cell = ch.bitrate.take();
        assert!(matches!(cell, Some(BitrateConfig::Classic(_))));
    }

    #[test]
    fn codemask_exact_match_is_single_id_range() {
        // Full mask = match every ID bit → from == to == code.
        assert_eq!(codemask_to_range(0x100, 0x7FF, false), Ok((0x100, 0x100)));
        assert_eq!(
            codemask_to_range(0x1AB_CDEF, 0x1FFF_FFFF, true),
            Ok((0x1AB_CDEF, 0x1AB_CDEF))
        );
    }

    #[test]
    fn codemask_contiguous_low_bits_form_a_range() {
        // mask 0x7FC leaves the low 2 bits don't-care → 0x100..=0x103.
        assert_eq!(codemask_to_range(0x100, 0x7FC, false), Ok((0x100, 0x103)));
        // Accept-all (mask 0) → full standard ID space.
        assert_eq!(codemask_to_range(0x000, 0x000, false), Ok((0x000, 0x7FF)));
    }

    #[test]
    fn codemask_non_contiguous_dontcare_is_rejected() {
        // Don't-care bits 0b101 are not a contiguous low-order block.
        assert_eq!(
            codemask_to_range(0x000, 0x7FA, false),
            Err(CanError::NotImplemented)
        );
    }

    #[test]
    fn read_status_ok_is_error_active() {
        assert_eq!(
            pcan_status_to_bus_status(sys::PCAN_ERROR_OK),
            BusStatus::ERROR_ACTIVE
        );
    }

    #[test]
    fn read_status_non_bus_bits_default_to_error_active() {
        // QRCVEMPTY isn't a bus state — it must not surface as an error
        // from a "what's the bus state?" query.
        assert_eq!(
            pcan_status_to_bus_status(sys::PCAN_ERROR_QRCVEMPTY),
            BusStatus::ERROR_ACTIVE
        );
    }

    #[test]
    fn read_status_recognises_bus_state_bits() {
        assert!(pcan_status_to_bus_status(sys::PCAN_ERROR_BUSOFF).contains(BusStatus::BUS_OFF));
        assert!(pcan_status_to_bus_status(sys::PCAN_ERROR_BUSPASSIVE)
            .contains(BusStatus::ERROR_PASSIVE));
        assert!(pcan_status_to_bus_status(sys::PCAN_ERROR_BUSWARNING)
            .contains(BusStatus::ERROR_WARNING));
    }

    #[test]
    fn read_status_overrun_from_either_queue_bit() {
        assert!(pcan_status_to_bus_status(sys::PCAN_ERROR_OVERRUN).contains(BusStatus::OVERRUN));
        assert!(pcan_status_to_bus_status(sys::PCAN_ERROR_QOVERRUN).contains(BusStatus::OVERRUN));
    }
}
