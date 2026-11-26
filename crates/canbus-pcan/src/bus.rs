//! The PCAN [`CanBus`] implementation.

use std::cell::Cell;
use std::ffi::CString;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_void};
use std::time::Duration;

use canbus_core::{
    dlc_to_len, len_to_dlc, BusConfig, BusStatus, CanBus, CanMessage, Capabilities, ChannelInfo,
    Deadline, Error, Filter, FilterSupport, Result, CANFD_MAX_LEN, CAN_MAX_LEN,
};

use crate::channel_name;
use crate::convert::{
    bus_state_from_pcan, c_buf_to_string, check, classic_btr, fd_bitrate_string, flags_to_pcan,
    is_ok, lib, read_flags,
};
use crate::event::{self, wait_for_event, EventHandle, WaitOutcome};
use crate::ffi as sys;
use crate::NAME;

/// An open PCAN channel.
///
/// The channel is initialised (and on-bus) once opened; it is uninitialised
/// when shut down or dropped.
///
/// # Threads
///
/// `PcanBus` is `Send` but not `Sync`, like every `canbus` backend.
#[derive(Debug)]
pub struct PcanBus {
    handle: sys::TPCANHandle,
    channel: String,
    fd: bool,
    listen_only: bool,
    event: Option<EventHandle>,
    open: bool,
    _not_sync: PhantomData<Cell<()>>,
}

// SAFETY: the only non-`Send` field is the Windows receive-event HANDLE (a
// raw pointer), which Win32 allows any thread to wait on or close.
// PCAN-Basic itself is thread-safe. `PhantomData<Cell<()>>` keeps the type
// `!Sync`.
unsafe impl Send for PcanBus {}

impl PcanBus {
    /// Open and initialise a PCAN channel. See the crate docs for the
    /// accepted configuration.
    pub fn open(config: &BusConfig) -> Result<Self> {
        let handle = channel_name::parse(&config.channel).ok_or_else(|| {
            Error::InvalidConfig(format!(
                "PCAN channel must be a name such as \"PCAN_USBBUS1\", got `{}`",
                config.channel
            ))
        })?;
        // Validate the bitrate before touching the driver.
        let init = match config.fd {
            Some(_) => Init::Fd(
                CString::new(fd_bitrate_string(config)?)
                    .map_err(|_| Error::InvalidConfig("NUL in bitrate string".into()))?,
            ),
            None => Init::Classic(classic_btr(config)?),
        };
        let lib = lib()?;

        if config.listen_only {
            let mut on = sys::PCAN_PARAMETER_ON;
            // SAFETY: valid buffer of the size passed.
            check(lib, unsafe {
                (lib.CAN_SetValue)(
                    handle,
                    sys::PCAN_LISTEN_ONLY,
                    &mut on as *mut u32 as *mut c_void,
                    4,
                )
            })?;
        }

        // SAFETY: plain values / a NUL-terminated string that outlives the call.
        let status = unsafe {
            match &init {
                Init::Classic(btr) => (lib.CAN_Initialize)(handle, *btr, 0, 0, 0),
                Init::Fd(s) => (lib.CAN_InitializeFD)(handle, s.as_ptr() as *mut c_char),
            }
        };
        check(lib, status)?;

        Ok(Self {
            handle,
            channel: channel_name::name(handle),
            fd: config.fd.is_some(),
            listen_only: config.listen_only,
            event: event::acquire(lib, handle),
            open: true,
            _not_sync: PhantomData,
        })
    }

    fn ensure_open(&self) -> Result<()> {
        if self.open {
            Ok(())
        } else {
            Err(Error::Shutdown)
        }
    }

    /// Read one queued frame without waiting.
    fn read_once(&self, lib: &sys::PcanLib) -> Result<Option<CanMessage>> {
        // SAFETY (both branches): valid handle; out-pointers refer to live locals.
        if self.fd {
            let mut m = sys::TPCANMsgFD::default();
            let mut ts: sys::TPCANTimestampFD = 0;
            let status = unsafe { (lib.CAN_ReadFD)(self.handle, &mut m, &mut ts) };
            if status & sys::PCAN_ERROR_QRCVEMPTY != 0 {
                return Ok(None);
            }
            check(lib, status)?;
            // PCAN reports the 4-bit DLC code; convert it to a byte length.
            let fd = m.MSGTYPE & sys::PCAN_MESSAGE_FD != 0;
            let len = dlc_to_len(m.DLC, fd);
            let data = &m.DATA[..len.min(CANFD_MAX_LEN)];
            let flags = read_flags(m.MSGTYPE, data);
            Ok(Some(CanMessage::from_raw(
                m.ID,
                data,
                len as u8,
                flags,
                Some(Duration::from_micros(ts)),
            )))
        } else {
            let mut m = sys::TPCANMsg::default();
            let mut ts = sys::TPCANTimestamp::default();
            let status = unsafe { (lib.CAN_Read)(self.handle, &mut m, &mut ts) };
            if status & sys::PCAN_ERROR_QRCVEMPTY != 0 {
                return Ok(None);
            }
            check(lib, status)?;
            let len = (m.LEN as usize).min(CAN_MAX_LEN);
            let data = &m.DATA[..len];
            let flags = read_flags(m.MSGTYPE, data);
            Ok(Some(CanMessage::from_raw(
                m.ID,
                data,
                len as u8,
                flags,
                Some(Duration::from_micros(ts.total_micros())),
            )))
        }
    }

    fn write_once(&self, lib: &sys::PcanLib, msg: &CanMessage) -> sys::TPCANStatus {
        let src = msg.data();
        // SAFETY (both branches): valid handle; the message is a live local.
        if self.fd {
            let len = src.len().min(CANFD_MAX_LEN);
            let mut m = sys::TPCANMsgFD {
                ID: msg.id(),
                MSGTYPE: flags_to_pcan(msg.flags()),
                DLC: len_to_dlc(msg.len()),
                ..Default::default()
            };
            m.DATA[..len].copy_from_slice(&src[..len]);
            unsafe { (lib.CAN_WriteFD)(self.handle, &mut m) }
        } else {
            let len = src.len().min(CAN_MAX_LEN);
            let mut m = sys::TPCANMsg {
                ID: msg.id(),
                MSGTYPE: flags_to_pcan(msg.flags()),
                LEN: msg.len().min(CAN_MAX_LEN) as u8,
                ..Default::default()
            };
            m.DATA[..len].copy_from_slice(&src[..len]);
            unsafe { (lib.CAN_Write)(self.handle, &mut m) }
        }
    }

    fn set_filter_mode(&self, lib: &sys::PcanLib, mode: u32) -> Result<()> {
        let mut mode = mode;
        // SAFETY: valid buffer of the size passed.
        check(lib, unsafe {
            (lib.CAN_SetValue)(
                self.handle,
                sys::PCAN_MESSAGE_FILTER,
                &mut mode as *mut u32 as *mut c_void,
                4,
            )
        })
    }
}

enum Init {
    Classic(sys::TPCANBaudrate),
    Fd(CString),
}

impl CanBus for PcanBus {
    fn send(&mut self, msg: &CanMessage, timeout: Option<Duration>) -> Result<()> {
        self.ensure_open()?;
        if self.listen_only {
            return Err(Error::Unsupported("send on a listen-only bus"));
        }
        if msg.is_fd() && !self.fd {
            return Err(Error::InvalidFrame(
                "CAN FD frame on a bus opened without CAN FD",
            ));
        }
        let lib = lib()?;
        // PCAN has no blocking write; poll until the queue accepts the frame.
        let deadline = Deadline::after(timeout);
        loop {
            match check(lib, self.write_once(lib, msg)) {
                Err(Error::TxQueueFull) if timeout == Some(Duration::ZERO) => {
                    return Err(Error::TxQueueFull)
                }
                Err(Error::TxQueueFull) if deadline.expired() => return Err(Error::Timeout),
                Err(Error::TxQueueFull) => std::thread::sleep(Duration::from_millis(1)),
                other => return other,
            }
        }
    }

    fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<CanMessage>> {
        self.ensure_open()?;
        let lib = lib()?;
        let deadline = Deadline::after(timeout);
        loop {
            if let Some(msg) = self.read_once(lib)? {
                return Ok(Some(msg));
            }
            let remaining = deadline.remaining();
            if remaining.is_some_and(|r| r.is_zero()) {
                return Ok(None);
            }
            // Block on the receive event when there is one, otherwise poll.
            // A spurious wake-up just loops back to the read.
            let wait = remaining.unwrap_or(Duration::from_secs(3600));
            match self.event {
                Some(ev) => match wait_for_event(ev, wait) {
                    WaitOutcome::Signalled | WaitOutcome::Timeout => {}
                    WaitOutcome::Failed => return Err(Error::Driver),
                },
                None => std::thread::sleep(Duration::from_millis(1).min(wait)),
            }
        }
    }

    /// PCAN filters on one ID range. An empty list is enforced in hardware.
    /// A single range (or a code/mask that forms one) is applied in
    /// hardware, but frames of the other ID format may still pass, so the
    /// result is [`FilterSupport::Software`]; more filters fall back to
    /// software.
    fn set_filters(&mut self, filters: &[Filter]) -> Result<FilterSupport> {
        self.ensure_open()?;
        let lib = lib()?;
        let range = match filters {
            [] => {
                self.set_filter_mode(lib, sys::PCAN_FILTER_OPEN)?;
                return Ok(FilterSupport::Hardware);
            }
            [Filter::Range { from, to, extended }] => Some((*from, *to, *extended)),
            [Filter::CodeMask {
                code,
                mask,
                extended,
            }] => codemask_to_range(*code, *mask, *extended).map(|(f, t)| (f, t, *extended)),
            _ => None,
        };
        match range {
            Some((from, to, extended)) if from <= to => {
                // Closing first makes the next call set exactly this range
                // instead of widening the previous one.
                self.set_filter_mode(lib, sys::PCAN_FILTER_CLOSE)?;
                let mode = if extended {
                    sys::PCAN_MODE_EXTENDED
                } else {
                    sys::PCAN_MODE_STANDARD
                };
                // SAFETY: valid handle, plain values.
                check(lib, unsafe {
                    (lib.CAN_FilterMessages)(self.handle, from, to, mode)
                })?;
            }
            _ => self.set_filter_mode(lib, sys::PCAN_FILTER_OPEN)?,
        }
        Ok(FilterSupport::Software)
    }

    fn status(&self) -> Result<BusStatus> {
        self.ensure_open()?;
        let lib = lib()?;
        // SAFETY: valid handle.
        let raw = unsafe { (lib.CAN_GetStatus)(self.handle) };
        Ok(BusStatus {
            state: bus_state_from_pcan(raw),
            error_counters: None,
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
            .with_fd(self.fd)
            .with_hardware_filters(true)
            .with_listen_only(true)
    }

    fn channel_info(&self) -> &str {
        &self.channel
    }

    // flush_tx: PCAN's only flush (`CAN_Reset`) also discards received
    // frames, so the default `Unsupported` is kept.

    fn shutdown(&mut self) -> Result<()> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        let lib = lib()?;
        if let Some(ev) = self.event.take() {
            event::release(lib, self.handle, ev);
        }
        // SAFETY: valid handle; never used again.
        check(lib, unsafe { (lib.CAN_Uninitialize)(self.handle) })
    }
}

impl Drop for PcanBus {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Translate a code/mask filter into an inclusive range, when the
/// don't-care bits within the ID width form one contiguous low-order block.
pub(crate) fn codemask_to_range(code: u32, mask: u32, extended: bool) -> Option<(u32, u32)> {
    let id_bits: u32 = if extended { 0x1FFF_FFFF } else { 0x7FF };
    let inv = !mask & id_bits;
    // `inv` must be of the form (1 << n) - 1, i.e. only the low n bits.
    if inv & inv.wrapping_add(1) != 0 {
        return None;
    }
    Some((code & mask & id_bits, (code | inv) & id_bits))
}

/// List the PCAN channels attached to this machine (available or occupied).
pub fn detect() -> Result<Vec<ChannelInfo>> {
    let lib = lib()?;
    let mut count: u32 = 0;
    // SAFETY: valid buffer of the size passed.
    check(lib, unsafe {
        (lib.CAN_GetValue)(
            sys::PCAN_NONEBUS,
            sys::PCAN_ATTACHED_CHANNELS_COUNT,
            &mut count as *mut u32 as *mut c_void,
            4,
        )
    })?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut raw = vec![sys::TPCANChannelInformation::default(); count as usize];
    let bytes = raw.len() * std::mem::size_of::<sys::TPCANChannelInformation>();
    // SAFETY: `raw` holds `count` entries, `bytes` long.
    let status = unsafe {
        (lib.CAN_GetValue)(
            sys::PCAN_NONEBUS,
            sys::PCAN_ATTACHED_CHANNELS,
            raw.as_mut_ptr() as *mut c_void,
            bytes as u32,
        )
    };
    if !is_ok(status) {
        check(lib, status)?;
    }
    Ok(raw
        .into_iter()
        .map(|info| ChannelInfo {
            interface: NAME,
            channel: channel_name::name(info.channel_handle),
            description: format!(
                "{} (controller {})",
                c_buf_to_string(&info.device_name),
                info.controller_number
            ),
            serial: Some(info.device_id.to_string()),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}

    #[test]
    fn bus_is_send() {
        assert_send::<PcanBus>();
    }

    #[test]
    fn codemask_exact_match_is_single_id_range() {
        assert_eq!(codemask_to_range(0x100, 0x7FF, false), Some((0x100, 0x100)));
        assert_eq!(
            codemask_to_range(0x1AB_CDEF, 0x1FFF_FFFF, true),
            Some((0x1AB_CDEF, 0x1AB_CDEF))
        );
    }

    #[test]
    fn codemask_contiguous_low_bits_form_a_range() {
        assert_eq!(codemask_to_range(0x100, 0x7FC, false), Some((0x100, 0x103)));
        assert_eq!(codemask_to_range(0x000, 0x000, false), Some((0x000, 0x7FF)));
    }

    #[test]
    fn codemask_non_contiguous_dontcare_is_rejected() {
        assert_eq!(codemask_to_range(0x000, 0x7FA, false), None);
    }

    #[test]
    fn invalid_channel_is_rejected_before_loading() {
        let err = PcanBus::open(&BusConfig::new(NAME, "usb1").bitrate(500_000)).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)));
        let err = PcanBus::open(&BusConfig::new(NAME, "PCAN_USBBUS1")).unwrap_err();
        assert!(matches!(err, Error::InvalidConfig(_)), "missing bitrate");
    }
}
