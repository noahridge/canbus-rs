use core::time::Duration;
use std::cell::Cell;
use std::marker::PhantomData;

use can_core::{
    impl_can_channel, impl_embedded_can, id_to_raw_and_flag,
    Bitrate, BusParams, BusStatistics, BusStatus, CanError, CanMessage, ErrorCounters, Filter,
    Result, CANFD_MAX_DLC,
};
use embedded_can::Id;
use pcan_sys as sys;

use crate::bitrate::{bitrate_to_btr, PcanFdBitrate};
use crate::bus::PcanBus;
use crate::error::{check_status, from_pcan_status, lib};
use crate::event::{wait_for_event, EventHandle, WaitOutcome};
use crate::flags::{fd_dlc_to_len, fd_len_to_dlc, flags_from_pcan, flags_to_pcan};

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
/// equivalent. To match the [`CanChannel`](can_core::CanChannel) trait
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
        // Cache the receive-event handle for use by read_wait. If the
        // backend doesn't support events (older drivers), leave it None
        // and fall back to polling.
        if let Some(h) = fetch_receive_event(l, self.handle) {
            self.event.set(Some(h));
        }
        Ok(())
    }

    pub fn bus_off(&mut self) -> Result<()> {
        if !self.on_bus {
            return Ok(());
        }
        let l = lib()?;
        check_status(unsafe { (l.CAN_Uninitialize)(self.handle) })?;
        self.on_bus = false;
        self.event.set(None);
        Ok(())
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
            // Re-initialise with new bitrate.
            let l = lib()?;
            check_status(unsafe { (l.CAN_Uninitialize)(self.handle) })?;
            check_status(unsafe { (l.CAN_Initialize)(self.handle, btr, 0, 0, 0) })?;
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
            let l = lib()?;
            check_status(unsafe { (l.CAN_Uninitialize)(self.handle) })?;
            let ptr = bitrate.0.as_ptr() as *mut std::os::raw::c_char;
            check_status(unsafe { (l.CAN_InitializeFD)(self.handle, ptr) })?;
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
            let mut m = sys::TPCANMsgFD::default();
            m.ID = msg.id();
            m.MSGTYPE = flags_to_pcan(msg.flags());
            let src = msg.data();
            let len = src.len().min(CANFD_MAX_DLC);
            m.DATA[..len].copy_from_slice(&src[..len]);
            m.DLC = fd_len_to_dlc(len);
            check_status(unsafe { (l.CAN_WriteFD)(self.handle, &mut m) })
        } else {
            let mut m = sys::TPCANMsg::default();
            m.ID = msg.id();
            m.MSGTYPE = flags_to_pcan(msg.flags());
            let src = msg.data();
            let len = src.len().min(8);
            m.DATA[..len].copy_from_slice(&src[..len]);
            m.LEN = len as u8;
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
            let len = fd_dlc_to_len(m.DLC);
            Ok(CanMessage::from_raw(
                m.ID,
                m.DATA[..len].to_vec(),
                m.DLC,
                flags_from_pcan(m.MSGTYPE),
                ts,
            ))
        } else {
            let mut m = sys::TPCANMsg::default();
            let mut ts = sys::TPCANTimestamp::default();
            check_status(unsafe { (l.CAN_Read)(self.handle, &mut m, &mut ts) })?;
            let len = m.LEN as usize;
            Ok(CanMessage::from_raw(
                m.ID,
                m.DATA[..len].to_vec(),
                m.LEN,
                flags_from_pcan(m.MSGTYPE),
                ts.total_micros(),
            ))
        }
    }

    pub fn read_wait(&self, timeout: Duration) -> Result<CanMessage> {
        // Fast path: a message is already queued.
        match self.read() {
            Ok(msg) => return Ok(msg),
            Err(CanError::NoMsg) => {}
            Err(e) => return Err(e),
        }

        // Event-driven wait, with polling fallback if no event handle.
        if let Some(h) = self.event.get() {
            match wait_for_event(h, timeout) {
                WaitOutcome::Signalled => self.read(),
                WaitOutcome::Timeout => Err(CanError::Timeout),
                WaitOutcome::Failed => Err(CanError::Driver),
            }
        } else {
            let deadline = std::time::Instant::now() + timeout;
            loop {
                match self.read() {
                    Ok(msg) => return Ok(msg),
                    Err(CanError::NoMsg) => {
                        if std::time::Instant::now() >= deadline {
                            return Err(CanError::Timeout);
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => return Err(e),
                }
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
            Filter::CodeMask { code, mask, extended } => {
                // Translate code/mask to a range when the masked bits form
                // a single contiguous low-order block. PCAN can then accept
                // it natively.
                let inv = !mask;
                // Check `inv` is of the form (1<<n)-1 — i.e. only the low n bits.
                if inv & inv.wrapping_add(1) != 0 {
                    return Err(CanError::NotImplemented);
                }
                let id_bits: u32 = if extended { 0x1FFF_FFFF } else { 0x7FF };
                let from = code & mask & id_bits;
                let to = (code | inv) & id_bits;
                (from, to, extended)
            }
        };
        let mode = if extended { sys::PCAN_MODE_EXTENDED } else { sys::PCAN_MODE_STANDARD };
        check_status(unsafe { (l.CAN_FilterMessages)(self.handle, from, to, mode) })
    }

    pub fn read_status(&self) -> Result<BusStatus> {
        let l = lib()?;
        let raw = unsafe { (l.CAN_GetStatus)(self.handle) };
        // PCAN_ERROR_OK means error-active; other bus bits map directly.
        let mut out = BusStatus::empty();
        if raw == sys::PCAN_ERROR_OK {
            return Ok(BusStatus::ERROR_ACTIVE);
        }
        if raw & sys::PCAN_ERROR_BUSOFF != 0 { out |= BusStatus::BUS_OFF; }
        if raw & sys::PCAN_ERROR_BUSPASSIVE != 0 { out |= BusStatus::ERROR_PASSIVE; }
        if raw & sys::PCAN_ERROR_BUSWARNING != 0 { out |= BusStatus::ERROR_WARNING; }
        if raw & (sys::PCAN_ERROR_OVERRUN | sys::PCAN_ERROR_QOVERRUN) != 0 {
            out |= BusStatus::OVERRUN;
        }
        if out.is_empty() {
            // Some other status — propagate as a backend error rather than
            // pretending the bus is in any particular state.
            return Err(from_pcan_status(raw));
        }
        Ok(out)
    }

    pub fn read_error_counters(&self) -> Result<ErrorCounters> {
        // PCAN-Basic does not expose TX/RX/overrun error counters directly.
        Err(CanError::NotImplemented)
    }

    pub fn flush_rx(&self) -> Result<()> {
        // CAN_Reset clears both queues — we expose it as flush_rx and
        // accept that flush_tx will also flush rx as a consequence.
        let l = lib()?;
        check_status(unsafe { (l.CAN_Reset)(self.handle) })
    }

    pub fn flush_tx(&self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.CAN_Reset)(self.handle) })
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
                unsafe {
                    (l.CAN_Uninitialize)(self.handle);
                }
            }
        }
    }
}

/// Try to fetch the platform receive-event handle for a channel.
///
/// Returns `None` if the call fails (driver doesn't support events on
/// this device, channel not initialised, etc.).
fn fetch_receive_event(l: &sys::PcanLib, handle: sys::TPCANHandle) -> Option<EventHandle> {
    let mut h: EventHandle = unsafe { core::mem::zeroed() };
    let status = unsafe {
        (l.CAN_GetValue)(
            handle,
            sys::PCAN_RECEIVE_EVENT,
            &mut h as *mut EventHandle as *mut std::os::raw::c_void,
            core::mem::size_of::<EventHandle>() as u32,
        )
    };
    if status == sys::PCAN_ERROR_OK { Some(h) } else { None }
}

// Pull in id_to_raw_and_flag use to satisfy the macro-generated trait code.
#[allow(dead_code)]
fn _unused_imports(id: Id) -> u32 {
    let (raw, _flag) = id_to_raw_and_flag(id);
    raw
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
}
