use std::marker::PhantomData;
use std::time::Duration;

use crate::ffi as sys;
use canbus_core::{
    id_to_raw_and_flag, impl_can_channel, impl_embedded_can, Bitrate, BusParams, BusParamsTq,
    BusStatistics, BusStatus, CanError, CanMessage, DriverType, ErrorCounters, FdBitrate, Filter,
    Result, CANFD_MAX_DLC,
};
use embedded_can::Id;

use crate::bus_params::{
    bitrate_to_raw, bus_params_tq_from_raw, bus_params_tq_to_raw, driver_type_from_raw,
    driver_type_to_raw, fd_bitrate_to_raw,
};
use crate::error::{check_handle, check_status, lib};
use crate::message::{flags_from_canlib, flags_to_canlib};
use crate::status::{bus_statistics_from_raw, bus_status_from_canlib};

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
        Ok(Self {
            handle,
            _not_send: PhantomData,
        })
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
        check_status(unsafe {
            (l.canSetBusParamsTq)(self.inner.raw(), bus_params_tq_to_raw(*params))
        })
    }

    pub fn set_fd_bitrate(&self, bitrate: FdBitrate) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusParamsFd)(self.inner.raw(), fd_bitrate_to_raw(bitrate), 0, 0, 0)
        })
    }

    pub fn set_bus_params_fd(&self, freq_brs: i64, tseg1: u32, tseg2: u32, sjw: u32) -> Result<()> {
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
        Ok(BusParams::new(
            freq as i64,
            tseg1,
            tseg2,
            sjw,
            no_samp,
            sync_mode,
        ))
    }

    pub fn get_bus_params_tq(&self) -> Result<BusParamsTq> {
        let l = lib()?;
        let mut raw = sys::kvBusParamsTq::default();
        check_status(unsafe { (l.canGetBusParamsTq)(self.inner.raw(), &mut raw) })?;
        Ok(bus_params_tq_from_raw(raw))
    }

    pub fn set_output_control(&self, driver: DriverType) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canSetBusOutputControl)(self.inner.raw(), driver_type_to_raw(driver))
        })
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
            (l.canWriteSync)(
                self.inner.raw(),
                timeout.as_millis() as std::os::raw::c_ulong,
            )
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

    pub fn read_wait(&self, timeout: Duration) -> Result<CanMessage> {
        let l = lib()?;
        let mut id: std::os::raw::c_long = 0;
        let mut data = [0u8; CANFD_MAX_DLC];
        let mut dlc: u32 = 0;
        let mut flags: u32 = 0;
        let mut timestamp: std::os::raw::c_ulong = 0;

        check_status(unsafe {
            (l.canReadWait)(
                self.inner.raw(),
                &mut id,
                data.as_mut_ptr() as *mut std::os::raw::c_void,
                &mut dlc,
                &mut flags,
                &mut timestamp,
                timeout.as_millis() as std::os::raw::c_ulong,
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

    pub fn read_sync(&self, timeout: Duration) -> Result<()> {
        let l = lib()?;
        check_status(unsafe {
            (l.canReadSync)(
                self.inner.raw(),
                timeout.as_millis() as std::os::raw::c_ulong,
            )
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
            Filter::CodeMask {
                code,
                mask,
                extended,
            } => (code, mask, extended),
            Filter::Range { from, to, extended } => {
                let (code, mask) = range_to_code_mask(from, to, extended)?;
                (code, mask, extended)
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
        Ok(bus_status_from_canlib(flags))
    }

    pub fn read_error_counters(&self) -> Result<ErrorCounters> {
        let l = lib()?;
        let mut tx: u32 = 0;
        let mut rx: u32 = 0;
        let mut ov: u32 = 0;
        check_status(unsafe {
            (l.canReadErrorCounters)(self.inner.raw(), &mut tx, &mut rx, &mut ov)
        })?;
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

    /// Get the raw CANLib handle (for advanced use with the raw CANlib API).
    pub fn raw_handle(&self) -> sys::canHandle {
        self.inner.raw()
    }
}

impl_can_channel!(Channel);
impl_embedded_can!(Channel);

/// Translate an inclusive ID range into a Kvaser code/mask pair.
///
/// A range is exactly representable as code/mask only when its size is a
/// power of two and `from` is aligned to that size (e.g. `0x100..=0x103`).
/// Anything else would make the filter accept IDs outside the range (or
/// reject IDs inside it), so it returns [`CanError::NotImplemented`].
fn range_to_code_mask(from: u32, to: u32, extended: bool) -> Result<(u32, u32)> {
    let id_bits: u32 = if extended { 0x1FFF_FFFF } else { 0x7FF };
    if from > to || to > id_bits {
        return Err(CanError::Param);
    }
    let size = to - from + 1;
    if !size.is_power_of_two() || from & (size - 1) != 0 {
        return Err(CanError::NotImplemented);
    }
    let mask = !(size - 1) & id_bits;
    Ok((from & mask, mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_range_maps_to_code_mask() {
        assert_eq!(range_to_code_mask(0x100, 0x103, false), Ok((0x100, 0x7FC)));
        assert_eq!(range_to_code_mask(0x123, 0x123, false), Ok((0x123, 0x7FF)));
        assert_eq!(range_to_code_mask(0, 0x7FF, false), Ok((0, 0)));
        assert_eq!(range_to_code_mask(0, 0x1FFF_FFFF, true), Ok((0, 0)));
    }

    #[test]
    fn unaligned_or_odd_sized_range_is_rejected() {
        // Previously mistranslated: 1..=3 produced a filter that also accepted 0.
        assert_eq!(
            range_to_code_mask(1, 3, false),
            Err(CanError::NotImplemented)
        );
        assert_eq!(
            range_to_code_mask(7, 15, false),
            Err(CanError::NotImplemented)
        );
        assert_eq!(
            range_to_code_mask(0, 2, false),
            Err(CanError::NotImplemented)
        );
    }

    #[test]
    fn invalid_range_is_param_error() {
        assert_eq!(range_to_code_mask(5, 4, false), Err(CanError::Param));
        assert_eq!(range_to_code_mask(0, 0x800, false), Err(CanError::Param));
    }

    #[test]
    fn accepted_ranges_match_exactly() {
        // Exhaustive check over small standard IDs: whenever a range is
        // accepted, the code/mask filter must pass exactly the IDs in range.
        for from in 0..0x40u32 {
            for to in from..0x40 {
                if let Ok((code, mask)) = range_to_code_mask(from, to, false) {
                    for id in 0..=0x7FFu32 {
                        assert_eq!(
                            id & mask == code,
                            (from..=to).contains(&id),
                            "range {from:#x}..={to:#x}, id {id:#x}"
                        );
                    }
                }
            }
        }
    }
}
