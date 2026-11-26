//! The Kvaser [`CanBus`] implementation.

use std::cell::Cell;
use std::marker::PhantomData;
use std::os::raw::{c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::Once;
use std::time::Duration;

use canbus_core::{
    BusConfig, BusStatus, CanBus, CanMessage, Capabilities, ChannelInfo, Deadline, Error,
    ErrorCounters, Filter, FilterSupport, MessageFlags, Result, CANFD_MAX_LEN, CAN_MAX_LEN,
};

use crate::convert::{
    bus_state_from_canlib, c_buf_to_string, check, classic_preset, error_from_status,
    fd_data_preset, flags_from_canlib, flags_to_canlib, lib, range_to_code_mask, timeout_ms,
};
use crate::ffi as sys;
use crate::NAME;

/// Timestamp resolution requested from the driver, in microseconds.
const WANTED_TIMER_SCALE_US: u32 = 10;

fn ensure_initialized(lib: &sys::CanLib) {
    static INIT: Once = Once::new();
    // SAFETY: canInitializeLibrary takes no arguments and may be called once
    // before any other CANlib function.
    INIT.call_once(|| unsafe { (lib.canInitializeLibrary)() });
}

/// An open Kvaser channel.
///
/// Opened on-bus; closed when shut down or dropped.
///
/// # Threads
///
/// `KvaserBus` is `Send` but not `Sync`. Kvaser's "Using Threads" guide
/// forbids calling CANlib concurrently on one handle but allows a handle to
/// be used from another thread with external mutual exclusion; owning the
/// bus and calling through `&mut self` provides exactly that.
#[derive(Debug)]
pub struct KvaserBus {
    handle: sys::canHandle,
    channel: String,
    fd: bool,
    listen_only: bool,
    timer_scale_us: u64,
    /// Accumulated wrap-arounds of a 32-bit timestamp counter.
    timer_wraps: u64,
    last_timer: c_ulong,
    open: bool,
    _not_sync: PhantomData<Cell<()>>,
}

impl KvaserBus {
    /// Open a Kvaser channel. See the crate docs for the accepted options.
    pub fn open(config: &BusConfig) -> Result<Self> {
        let lib = lib()?;
        ensure_initialized(lib);

        let index: c_int = config.channel.trim().parse().map_err(|_| {
            Error::InvalidConfig(format!(
                "Kvaser channel must be a channel index such as \"0\", got `{}`",
                config.channel
            ))
        })?;

        let mut flags = 0;
        if config.bool_option("accept_virtual", true)? {
            flags |= sys::canOPEN_ACCEPT_VIRTUAL;
        }
        if config.bool_option("exclusive", false)? {
            flags |= sys::canOPEN_EXCLUSIVE;
        }
        if config.fd.is_some() {
            flags |= if config.bool_option("fd_non_iso", false)? {
                sys::canOPEN_CAN_FD_NONISO
            } else {
                sys::canOPEN_CAN_FD
            };
        }

        // SAFETY: plain value arguments.
        let handle = unsafe { (lib.canOpenChannel)(index, flags) };
        if handle < 0 {
            return Err(error_from_status(lib, handle));
        }
        // From here on `Drop` closes the handle if configuration fails.
        let mut bus = Self {
            handle,
            channel: config.channel.trim().to_owned(),
            fd: config.fd.is_some(),
            listen_only: config.listen_only,
            timer_scale_us: 1000,
            timer_wraps: 0,
            last_timer: 0,
            open: true,
            _not_sync: PhantomData,
        };
        bus.configure(lib, config)?;
        // SAFETY: `handle` is a valid open handle.
        check(lib, unsafe { (lib.canBusOn)(handle) })?;
        Ok(bus)
    }

    fn configure(&mut self, lib: &sys::CanLib, config: &BusConfig) -> Result<()> {
        let h = self.handle;
        // SAFETY (all calls below): `h` is a valid open handle and every
        // argument is a plain value or a pointer to a live local.
        if let Some(t) = config.timing {
            check(lib, unsafe {
                (lib.canSetBusParams)(h, t.bitrate() as c_long, t.tseg1, t.tseg2, t.sjw, 1, 0)
            })?;
        } else if let Some(bitrate) = config.bitrate {
            match classic_preset(bitrate) {
                Some(preset) => check(lib, unsafe {
                    (lib.canSetBusParams)(h, preset, 0, 0, 0, 0, 0)
                })?,
                None => {
                    // Let CANlib derive the timing for a non-standard rate.
                    let rate = c_int::try_from(bitrate).map_err(|_| {
                        Error::InvalidConfig(format!("bitrate {bitrate} is out of range"))
                    })?;
                    check(lib, unsafe { (lib.canSetBitrate)(h, rate) })?;
                }
            }
        }

        if let Some(fd) = config.fd {
            if let Some(t) = fd.data_timing {
                check(lib, unsafe {
                    (lib.canSetBusParamsFd)(h, t.bitrate() as c_long, t.tseg1, t.tseg2, t.sjw)
                })?;
            } else {
                let preset = fd_data_preset(fd.data_bitrate).ok_or_else(|| {
                    Error::InvalidConfig(format!(
                        "no Kvaser preset for CAN FD data bitrate {}; presets are 500k, 1M, \
                         2M, 4M and 8M, otherwise set explicit data timing",
                        fd.data_bitrate
                    ))
                })?;
                check(lib, unsafe { (lib.canSetBusParamsFd)(h, preset, 0, 0, 0) })?;
            }
        }

        if config.listen_only {
            check(lib, unsafe {
                (lib.canSetBusOutputControl)(h, sys::canDRIVER_SILENT)
            })?;
        }

        // Ask for 10 µs timestamps (best effort; not every device supports
        // it), then read back the resolution actually in use.
        let mut wanted = WANTED_TIMER_SCALE_US;
        let mut actual: u32 = 0;
        unsafe {
            (lib.canIoCtl)(
                h,
                sys::canIOCTL_SET_TIMER_SCALE,
                &mut wanted as *mut u32 as *mut c_void,
                4,
            );
            if (lib.canIoCtl)(
                h,
                sys::canIOCTL_GET_TIMER_SCALE,
                &mut actual as *mut u32 as *mut c_void,
                4,
            ) >= 0
                && actual > 0
            {
                self.timer_scale_us = u64::from(actual);
            }
        }
        Ok(())
    }

    fn ensure_open(&self) -> Result<()> {
        if self.open {
            Ok(())
        } else {
            Err(Error::Shutdown)
        }
    }

    /// Convert a raw timer value to a `Duration`, unwrapping 32-bit
    /// counter overflow (`c_ulong` is 32-bit on Windows). A gap of more than
    /// one full counter period between frames can't be detected.
    fn timestamp(&mut self, raw: c_ulong) -> Duration {
        if std::mem::size_of::<c_ulong>() == 4 && raw < self.last_timer {
            self.timer_wraps += 1;
        }
        self.last_timer = raw;
        #[allow(clippy::useless_conversion)] // c_ulong is u64 on some targets
        let ticks = u64::from(raw) + (self.timer_wraps << 32);
        Duration::from_micros(ticks.saturating_mul(self.timer_scale_us))
    }

    fn set_accept_all(&self, lib: &sys::CanLib) -> Result<()> {
        for extended in [0, 1] {
            // SAFETY: valid handle, plain values.
            check(lib, unsafe {
                (lib.canSetAcceptanceFilter)(self.handle, 0, 0, extended)
            })?;
        }
        Ok(())
    }
}

impl CanBus for KvaserBus {
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
        let mut data = [0u8; CANFD_MAX_LEN];
        let src = msg.data();
        let n = src.len().min(CANFD_MAX_LEN);
        data[..n].copy_from_slice(&src[..n]);
        // For CAN FD, CANlib takes the payload length in bytes, not the DLC code.
        let len = msg.len().min(CANFD_MAX_LEN) as c_uint;
        let flags = flags_to_canlib(msg.flags());

        let deadline = Deadline::after(timeout);
        loop {
            // SAFETY: valid handle; `data` outlives the call and holds 64 bytes.
            let status = unsafe {
                (lib.canWrite)(
                    self.handle,
                    msg.id() as c_long,
                    data.as_mut_ptr() as *mut c_void,
                    len,
                    flags,
                )
            };
            match status {
                sys::canERR_TXBUFOFL if timeout == Some(Duration::ZERO) => {
                    return Err(Error::TxQueueFull)
                }
                sys::canERR_TXBUFOFL if deadline.expired() => return Err(Error::Timeout),
                sys::canERR_TXBUFOFL => std::thread::sleep(Duration::from_millis(1)),
                s => return check(lib, s),
            }
        }
    }

    fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<CanMessage>> {
        self.ensure_open()?;
        let lib = lib()?;
        let mut id: c_long = 0;
        let mut data = [0u8; CANFD_MAX_LEN];
        let mut dlc: c_uint = 0;
        let mut raw_flags: c_uint = 0;
        let mut time: c_ulong = 0;

        // SAFETY: valid handle; every out-pointer refers to a live local and
        // `data` holds the 64 bytes CANlib may write for an FD frame.
        let status = unsafe {
            if timeout == Some(Duration::ZERO) {
                (lib.canRead)(
                    self.handle,
                    &mut id,
                    data.as_mut_ptr() as *mut c_void,
                    &mut dlc,
                    &mut raw_flags,
                    &mut time,
                )
            } else {
                (lib.canReadWait)(
                    self.handle,
                    &mut id,
                    data.as_mut_ptr() as *mut c_void,
                    &mut dlc,
                    &mut raw_flags,
                    &mut time,
                    timeout_ms(timeout),
                )
            }
        };
        match status {
            sys::canERR_NOMSG | sys::canERR_TIMEOUT => return Ok(None),
            s => check(lib, s)?,
        }

        let flags = flags_from_canlib(raw_flags);
        // CANlib reports the FD payload length in bytes. Classic frames can
        // carry a DLC above 8 (canOPEN_ACCEPT_LARGE_DLC) but never more data.
        let max = if flags.contains(MessageFlags::FD) {
            CANFD_MAX_LEN
        } else {
            CAN_MAX_LEN
        };
        let len = (dlc as usize).min(max);
        let timestamp = self.timestamp(time);
        Ok(Some(CanMessage::from_raw(
            id as u32,
            &data[..len],
            len as u8,
            flags,
            Some(timestamp),
        )))
    }

    /// Kvaser has one code/mask filter per frame format. An empty list is
    /// enforced in hardware; a single filter is applied in hardware but
    /// frames of the other format still pass, so the result is
    /// [`FilterSupport::Software`]; more filters fall back to software.
    fn set_filters(&mut self, filters: &[Filter]) -> Result<FilterSupport> {
        self.ensure_open()?;
        let lib = lib()?;
        self.set_accept_all(lib)?;
        let hw = match filters {
            [] => return Ok(FilterSupport::Hardware),
            [Filter::CodeMask {
                code,
                mask,
                extended,
            }] => Some((*code, *mask, *extended)),
            [Filter::Range { from, to, extended }] => {
                range_to_code_mask(*from, *to, *extended).map(|(c, m)| (c, m, *extended))
            }
            _ => None,
        };
        if let Some((code, mask, extended)) = hw {
            // SAFETY: valid handle, plain values.
            check(lib, unsafe {
                (lib.canSetAcceptanceFilter)(self.handle, code, mask, c_int::from(extended))
            })?;
        }
        Ok(FilterSupport::Software)
    }

    fn status(&self) -> Result<BusStatus> {
        self.ensure_open()?;
        let lib = lib()?;
        let mut bits: c_ulong = 0;
        let (mut tx, mut rx, mut overrun): (c_uint, c_uint, c_uint) = (0, 0, 0);
        // SAFETY: valid handle; out-pointers refer to live locals.
        unsafe {
            check(lib, (lib.canReadStatus)(self.handle, &mut bits))?;
            check(
                lib,
                (lib.canReadErrorCounters)(self.handle, &mut tx, &mut rx, &mut overrun),
            )?;
        }
        Ok(BusStatus {
            state: bus_state_from_canlib(bits),
            error_counters: Some(ErrorCounters { tx, rx }),
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

    fn flush_tx(&mut self) -> Result<()> {
        self.ensure_open()?;
        let lib = lib()?;
        // SAFETY: valid handle.
        check(lib, unsafe { (lib.canFlushTransmitQueue)(self.handle) })
    }

    fn shutdown(&mut self) -> Result<()> {
        if !self.open {
            return Ok(());
        }
        self.open = false;
        let lib = lib()?;
        // SAFETY: the handle is valid until canClose; it's never used again.
        let (off, close) = unsafe { ((lib.canBusOff)(self.handle), (lib.canClose)(self.handle)) };
        check(lib, off)?;
        check(lib, close)
    }
}

impl Drop for KvaserBus {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// List Kvaser channels, including virtual ones.
pub fn detect() -> Result<Vec<ChannelInfo>> {
    let lib = lib()?;
    ensure_initialized(lib);
    let mut count: c_int = 0;
    // SAFETY: out-pointer refers to a live local.
    check(lib, unsafe { (lib.canGetNumberOfChannels)(&mut count) })?;

    (0..count)
        .map(|index| {
            let mut name = [0u8; 256];
            let mut serial: u64 = 0;
            // SAFETY: buffers are live locals of the sizes passed.
            unsafe {
                check(
                    lib,
                    (lib.canGetChannelData)(
                        index,
                        sys::canCHANNELDATA_CHANNEL_NAME,
                        name.as_mut_ptr() as *mut c_void,
                        name.len(),
                    ),
                )?;
                check(
                    lib,
                    (lib.canGetChannelData)(
                        index,
                        sys::canCHANNELDATA_CARD_SERIAL_NO,
                        &mut serial as *mut u64 as *mut c_void,
                        std::mem::size_of::<u64>(),
                    ),
                )?;
            }
            Ok(ChannelInfo {
                interface: NAME,
                channel: index.to_string(),
                description: c_buf_to_string(&name),
                serial: Some(serial.to_string()),
            })
        })
        .collect()
}
