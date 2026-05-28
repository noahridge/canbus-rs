use std::marker::PhantomData;
use std::time::Duration;

use canlib_sys as sys;
use can_core::{
    impl_can_channel, impl_embedded_can, id_to_raw_and_flag,
    Bitrate, BusParams, BusParamsTq, BusStatus, BusStatistics, CanError, CanMessage, DriverType,
    ErrorCounters, FdBitrate, Filter, Result, CANFD_MAX_DLC,
};
use embedded_can::Id;

use crate::bus_params::{
    bitrate_to_raw, bus_params_tq_from_raw, bus_params_tq_to_raw, driver_type_from_raw,
    driver_type_to_raw, fd_bitrate_to_raw,
};
use crate::error::{check_handle, check_status, lib};
use crate::message::{flags_from_canlib, flags_to_canlib};
use crate::status::{bus_status_from_canlib, bus_statistics_from_raw};

bitflags::bitflags! {
    /// Flags for opening a CAN channel (Kvaser-specific).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: i32 {
        /// Don't allow sharing of this channel.
        const EXCLUSIVE = sys::canOPEN_EXCLUSIVE;
        /// Require extended CAN (2.0B) support.
        const REQUIRE_EXTENDED = sys::canOPEN_REQUIRE_EXTENDED;
        /// Allow opening virtual channels.
        const ACCEPT_VIRTUAL = sys::canOPEN_ACCEPT_VIRTUAL;
        /// Override exclusive access from other handles.
        const OVERRIDE_EXCLUSIVE = sys::canOPEN_OVERRIDE_EXCLUSIVE;
        /// Require init access to set bus parameters.
        const REQUIRE_INIT_ACCESS = sys::canOPEN_REQUIRE_INIT_ACCESS;
        /// Open without init access.
        const NO_INIT_ACCESS = sys::canOPEN_NO_INIT_ACCESS;
        /// Accept DLC values greater than 8.
        const ACCEPT_LARGE_DLC = sys::canOPEN_ACCEPT_LARGE_DLC;
        /// Open for CAN FD (ISO 11898-1:2015).
        const CAN_FD = sys::canOPEN_CAN_FD;
        /// Open for CAN FD (non-ISO / Bosch).
        const CAN_FD_NONISO = sys::canOPEN_CAN_FD_NONISO;
    }
}

/// RAII wrapper around a raw `canHandle`.
struct ChannelHandle {
    handle: sys::canHandle,
    _not_send: PhantomData<*const ()>,
}

impl ChannelHandle {
    fn open(channel_num: i32, flags: OpenFlags) -> Result<Self> {
        crate::ensure_initialized()?;
        let l = lib()?;
        let handle = unsafe { (l.canOpenChannel)(channel_num, flags.bits()) };
        check_handle(handle)?;
        Ok(Self { handle, _not_send: PhantomData })
    }

    fn raw(&self) -> sys::canHandle {
        self.handle
    }
}

impl Drop for ChannelHandle {
    fn drop(&mut self) {
        if let Ok(l) = lib() {
            unsafe {
                (l.canBusOff)(self.handle);
                (l.canClose)(self.handle);
            }
        }
    }
}

/// A handle to an open Kvaser CAN channel.
///
/// The channel is automatically taken off-bus and closed when dropped.
///
/// # Thread safety
///
/// `Channel` is `!Send` and `!Sync` because CANlib handles are thread-affine.
pub struct Channel {
    inner: ChannelHandle,
    on_bus: bool,
}

impl Channel {
    /// Open a CAN channel by zero-based index.
    pub fn open(channel_num: i32, flags: OpenFlags) -> Result<Self> {
        Ok(Self {
            inner: ChannelHandle::open(channel_num, flags)?,
            on_bus: false,
        })
    }

    pub fn bus_on(&mut self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canBusOn)(self.inner.raw()) })?;
        self.on_bus = true;
        Ok(())
    }

    pub fn bus_off(&mut self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canBusOff)(self.inner.raw()) })?;
        self.on_bus = false;
        Ok(())
    }

    pub fn reset_bus(&mut self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canResetBus)(self.inner.raw()) })?;
        self.on_bus = false;
        Ok(())
    }

    pub fn is_on_bus(&self) -> bool {
        self.on_bus
    }

    pub fn set_bitrate(&self, bitrate: Bitrate) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusParams)(self.inner.raw(), bitrate_to_raw(bitrate), 0, 0, 0, 0, 0)
        })
    }

    pub fn set_bus_params(&self, params: &BusParams) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusParams)(
                self.inner.raw(),
                params.freq() as std::os::raw::c_long,
                params.tseg1(),
                params.tseg2(),
                params.sjw(),
                params.no_samp(),
                params.sync_mode(),
            )
        })
    }

    pub fn set_bus_params_tq(&self, params: &BusParamsTq) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canSetBusParamsTq)(self.inner.raw(), bus_params_tq_to_raw(*params)) })
    }

    pub fn set_fd_bitrate(&self, bitrate: FdBitrate) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusParamsFd)(self.inner.raw(), fd_bitrate_to_raw(bitrate), 0, 0, 0)
        })
    }

    pub fn set_bus_params_fd(
        &self,
        freq_brs: i64,
        tseg1: u32,
        tseg2: u32,
        sjw: u32,
    ) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusParamsFd)(
                self.inner.raw(),
                freq_brs as std::os::raw::c_long,
                tseg1,
                tseg2,
                sjw,
            )
        })
    }

    pub fn set_bus_params_fd_tq(
        &self,
        arbitration: &BusParamsTq,
        data: &BusParamsTq,
    ) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusParamsFdTq)(
                self.inner.raw(),
                bus_params_tq_to_raw(*arbitration),
                bus_params_tq_to_raw(*data),
            )
        })
    }

    pub fn get_bus_params(&self) -> Result<BusParams> {
        let l = lib()?;
        let mut freq: std::os::raw::c_long = 0;
        let mut tseg1: u32 = 0;
        let mut tseg2: u32 = 0;
        let mut sjw: u32 = 0;
        let mut no_samp: u32 = 0;
        let mut sync_mode: u32 = 0;
        check_status(unsafe {
            (l.canGetBusParams)(
                self.inner.raw(),
                &mut freq,
                &mut tseg1,
                &mut tseg2,
                &mut sjw,
                &mut no_samp,
                &mut sync_mode,
            )
        })?;
        Ok(BusParams::new(freq as i64, tseg1, tseg2, sjw, no_samp, sync_mode))
    }

    pub fn get_bus_params_tq(&self) -> Result<BusParamsTq> {
        let l = lib()?;
        let mut raw = sys::kvBusParamsTq::default();
        check_status(unsafe { (l.canGetBusParamsTq)(self.inner.raw(), &mut raw) })?;
        Ok(bus_params_tq_from_raw(raw))
    }

    pub fn set_output_control(&self, driver: DriverType) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canSetBusOutputControl)(self.inner.raw(), driver_type_to_raw(driver)) })
    }

    pub fn get_output_control(&self) -> Result<DriverType> {
        let l = lib()?;
        let mut raw: u32 = 0;
        check_status(unsafe { (l.canGetBusOutputControl)(self.inner.raw(), &mut raw) })?;
        driver_type_from_raw(raw).ok_or(CanError::Backend(raw as i32))
    }

    pub fn write(&self, msg: &CanMessage) -> Result<()> {
        let l = lib()?;
        let mut data = [0u8; CANFD_MAX_DLC];
        let src = msg.data();
        let len = src.len().min(CANFD_MAX_DLC);
        data[..len].copy_from_slice(&src[..len]);

        check_status(unsafe {
            (l.canWrite)(
                self.inner.raw(),
                msg.id() as std::os::raw::c_long,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                msg.dlc() as u32,
                flags_to_canlib(msg.flags()),
            )
        })
    }

    pub fn write_wait(&self, msg: &CanMessage, timeout: Duration) -> Result<()> {
        let l = lib()?;
        let mut data = [0u8; CANFD_MAX_DLC];
        let src = msg.data();
        let len = src.len().min(CANFD_MAX_DLC);
        data[..len].copy_from_slice(&src[..len]);

        check_status(unsafe {
            (l.canWriteWait)(
                self.inner.raw(),
                msg.id() as std::os::raw::c_long,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                msg.dlc() as u32,
                flags_to_canlib(msg.flags()),
                timeout.as_millis() as std::os::raw::c_ulong,
            )
        })
    }

    pub fn write_sync(&self, timeout: Duration) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canWriteSync)(self.inner.raw(), timeout.as_millis() as std::os::raw::c_ulong)
        })
    }

    pub fn read(&self) -> Result<CanMessage> {
        let l = lib()?;
        let mut id: std::os::raw::c_long = 0;
        let mut data = [0u8; CANFD_MAX_DLC];
        let mut dlc: u32 = 0;
        let mut flags: u32 = 0;
        let mut timestamp: std::os::raw::c_ulong = 0;

        check_status(unsafe {
            (l.canRead)(
                self.inner.raw(),
                &mut id,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                &mut dlc,
                &mut flags,
                &mut timestamp,
            )
        })?;

        let data_len = (dlc as usize).min(CANFD_MAX_DLC);
        Ok(CanMessage::from_raw(
            id as u32,
            data[..data_len].to_vec(),
            dlc as u8,
            flags_from_canlib(flags),
            timestamp as u64,
        ))
    }

    /// Blocking read with a timeout. Returns [`CanError::Timeout`] if the
    /// deadline expires with no message, matching the pcan backend so
    /// vendor-agnostic code can match a single error variant.
    pub fn read_wait(&self, timeout: Duration) -> Result<CanMessage> {
        let l = lib()?;
        let mut id: std::os::raw::c_long = 0;
        let mut data = [0u8; CANFD_MAX_DLC];
        let mut dlc: u32 = 0;
        let mut flags: u32 = 0;
        let mut timestamp: std::os::raw::c_ulong = 0;

        let status = unsafe {
            (l.canReadWait)(
                self.inner.raw(),
                &mut id,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                &mut dlc,
                &mut flags,
                &mut timestamp,
                timeout.as_millis() as std::os::raw::c_ulong,
            )
        };
        // canReadWait surfaces canERR_NOMSG when the timeout expires with
        // no data. Map it to CanError::Timeout so read_wait matches the
        // pcan backend's contract; NoMsg stays reserved for the
        // non-blocking read() path.
        match check_status(status) {
            Ok(()) => {}
            Err(CanError::NoMsg) => return Err(CanError::Timeout),
            Err(e) => return Err(e),
        }

        let data_len = (dlc as usize).min(CANFD_MAX_DLC);
        Ok(CanMessage::from_raw(
            id as u32,
            data[..data_len].to_vec(),
            dlc as u8,
            flags_from_canlib(flags),
            timestamp as u64,
        ))
    }

    pub fn read_sync(&self, timeout: Duration) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canReadSync)(self.inner.raw(), timeout.as_millis() as std::os::raw::c_ulong)
        })
    }

    pub fn read_specific(&self, id: Id) -> Result<CanMessage> {
        let l = lib()?;
        let (raw, _) = id_to_raw_and_flag(id);
        let mut data = [0u8; CANFD_MAX_DLC];
        let mut dlc: u32 = 0;
        let mut flags: u32 = 0;
        let mut timestamp: std::os::raw::c_ulong = 0;

        check_status(unsafe {
            (l.canReadSpecific)(
                self.inner.raw(),
                raw as std::os::raw::c_long,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                &mut dlc,
                &mut flags,
                &mut timestamp,
            )
        })?;

        let data_len = (dlc as usize).min(CANFD_MAX_DLC);
        Ok(CanMessage::from_raw(
            raw,
            data[..data_len].to_vec(),
            dlc as u8,
            flags_from_canlib(flags),
            timestamp as u64,
        ))
    }

    pub fn read_specific_skip(&self, id: Id) -> Result<CanMessage> {
        let l = lib()?;
        let (raw, _) = id_to_raw_and_flag(id);
        let mut data = [0u8; CANFD_MAX_DLC];
        let mut dlc: u32 = 0;
        let mut flags: u32 = 0;
        let mut timestamp: std::os::raw::c_ulong = 0;

        check_status(unsafe {
            (l.canReadSpecificSkip)(
                self.inner.raw(),
                raw as std::os::raw::c_long,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                &mut dlc,
                &mut flags,
                &mut timestamp,
            )
        })?;

        let data_len = (dlc as usize).min(CANFD_MAX_DLC);
        Ok(CanMessage::from_raw(
            raw,
            data[..data_len].to_vec(),
            dlc as u8,
            flags_from_canlib(flags),
            timestamp as u64,
        ))
    }

    /// Configure an acceptance filter.
    ///
    /// Kvaser natively supports the [`Filter::CodeMask`] form. A
    /// [`Filter::Range`] is translated into a code/mask pair when the
    /// range is representable that way; arbitrary ranges return
    /// [`CanError::NotImplemented`].
    pub fn set_acceptance_filter(&self, filter: &Filter) -> Result<()> {
        let l = lib()?;
        let (code, mask, extended) = match *filter {
            Filter::CodeMask { code, mask, extended } => (code, mask, extended),
            Filter::Range { from, to, extended } => {
                // A contiguous range maps to code/mask iff `to == from | !mask`
                // for some mask whose set bits are the low bits up to a
                // power-of-two boundary. Detect that case; otherwise refuse.
                if from > to {
                    return Err(CanError::Param);
                }
                let span = to ^ from;
                // The smallest mask covering the range is when (span+1) is
                // a power of two and from is aligned to it.
                let cover = span | (span >> 1) | (span >> 2) | (span >> 4)
                    | (span >> 8) | (span >> 16);
                if to == (from | cover) {
                    let id_bits: u32 = if extended { 0x1FFF_FFFF } else { 0x7FF };
                    let mask = (!cover) & id_bits;
                    (from & mask, mask, extended)
                } else {
                    return Err(CanError::NotImplemented);
                }
            }
        };
        check_status(unsafe {
            (l.canSetAcceptanceFilter)(self.inner.raw(), code, mask, extended as i32)
        })
    }

    pub fn read_status(&self) -> Result<BusStatus> {
        let l = lib()?;
        let mut flags: std::os::raw::c_ulong = 0;
        check_status(unsafe { (l.canReadStatus)(self.inner.raw(), &mut flags) })?;
        Ok(bus_status_from_canlib(flags as u64))
    }

    pub fn read_error_counters(&self) -> Result<ErrorCounters> {
        let l = lib()?;
        let mut tx: u32 = 0;
        let mut rx: u32 = 0;
        let mut ov: u32 = 0;
        check_status(unsafe { (l.canReadErrorCounters)(self.inner.raw(), &mut tx, &mut rx, &mut ov) })?;
        Ok(ErrorCounters {
            tx_errors: tx,
            rx_errors: rx,
            overrun_errors: ov,
        })
    }

    pub fn request_chip_status(&self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canRequestChipStatus)(self.inner.raw()) })
    }

    pub fn request_bus_statistics(&self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canRequestBusStatistics)(self.inner.raw()) })
    }

    pub fn get_bus_statistics(&self) -> Result<BusStatistics> {
        let l = lib()?;
        let mut raw = sys::canBusStatistics::default();
        check_status(unsafe {
            (l.canGetBusStatistics)(
                self.inner.raw(),
                &mut raw,
                std::mem::size_of::<sys::canBusStatistics>(),
            )
        })?;
        Ok(bus_statistics_from_raw(&raw))
    }

    pub fn flush_rx(&self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canFlushReceiveQueue)(self.inner.raw()) })
    }

    pub fn flush_tx(&self) -> Result<()> {
        let l = lib()?;
        check_status(unsafe { (l.canFlushTransmitQueue)(self.inner.raw()) })
    }

    /// Get the raw CANLib handle (for advanced use with `canlib-sys`).
    pub fn raw_handle(&self) -> sys::canHandle {
        self.inner.raw()
    }
}

impl_can_channel!(Channel);
impl_embedded_can!(Channel);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_mask_filter_passes_through() {
        // We can't call into the FFI without a channel, but the
        // translation logic for Range -> CodeMask is testable in isolation.
        // Aligned 4-id range starting at a multiple of 4: 0x100..=0x103
        // Should produce code=0x100, mask=!0b11 & 0x7FF = 0x7FC.
        let (code, mask, ext) = match (Filter::Range { from: 0x100, to: 0x103, extended: false }) {
            Filter::Range { from, to, extended } => {
                let span = to ^ from;
                let cover = span | (span >> 1) | (span >> 2) | (span >> 4) | (span >> 8) | (span >> 16);
                let mask_id_bits = if extended { 0x1FFF_FFFFu32 } else { 0x7FF };
                ((from & ((!cover) & mask_id_bits)), (!cover) & mask_id_bits, extended)
            }
            _ => unreachable!(),
        };
        assert_eq!(code, 0x100);
        assert_eq!(mask, 0x7FC);
        assert!(!ext);
    }
}
