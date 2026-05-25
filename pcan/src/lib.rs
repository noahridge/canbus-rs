//! Safe, idiomatic Rust wrapper for the PEAK PCAN-Basic API.
//!
//! Implements the shared [`CanChannel`](can_core::CanChannel) trait set so
//! application code can be written once and run against Kvaser or PEAK
//! hardware. PCAN's lifecycle differs from Kvaser's — see [`PcanChannel`]
//! for details.
//!
//! # Getting started
//!
//! ```no_run
//! use pcan::{Bitrate, CanMessage, PcanBus, PcanChannel, StandardId};
//! use std::time::Duration;
//!
//! let mut ch = PcanChannel::open(PcanBus::Usb(1)).unwrap();
//! ch.set_bitrate(Bitrate::Rate500K).unwrap();
//! ch.bus_on().unwrap();
//!
//! let id = StandardId::new(0x123).unwrap();
//! let msg = CanMessage::new(id, &[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
//! ch.write(&msg).unwrap();
//!
//! match ch.read_wait(Duration::from_secs(1)) {
//!     Ok(rx) => println!("Received: id=0x{:X} data={:?}", rx.id(), rx.data()),
//!     Err(pcan::CanError::Timeout) => println!("No message received"),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```

pub mod bitrate;
pub mod bus;
pub mod channel;
pub mod error;
mod event;
pub mod flags;

// Re-export everything from can-core so users only need the `pcan`
// dependency to get the unified API surface.
pub use can_core::{
    blocking_receive, blocking_transmit, decode_error_frame, nb_receive, nb_transmit,
    Bitrate, Brs, BusError, BusErrorKind, BusParams, BusParamsTq, BusStatistics, BusStatus,
    CanBusControl, CanChannel, CanDiagnostics, CanError, CanMessage, CanRead, CanWrite,
    DataFrame, DriverType, EmbeddedCanError, ErrorCounters, ErrorFrame, ExtendedId, FdBitrate,
    Filter, Frame, Id, MessageFlags, RemoteFrame, Result, StandardId,
    CAN_EXT_ID_MAX, CAN_MAX_DLC, CAN_STD_ID_MAX, CANFD_MAX_DLC,
};

pub use can_core::embedded_can;

pub use bitrate::PcanFdBitrate;
pub use bus::PcanBus;
pub use channel::PcanChannel;
pub use error::{check_status, from_pcan_status};

use pcan_sys as sys;

/// Information about an attached PCAN channel.
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    /// Raw PCAN channel handle (`PCAN_USBBUS1` etc.).
    pub channel_handle: u16,
    /// Device kind (`PCAN_USB`, `PCAN_PCI`, `PCAN_LAN`, …).
    pub device_type: u8,
    /// Hardware CAN-controller number on the device.
    pub controller_number: u8,
    /// Bitmap of `FEATURE_*` flags.
    pub features: u32,
    /// Hardware name (e.g. `"PCAN-USB"`).
    pub device_name: String,
    /// Device serial / id.
    pub device_id: u32,
    /// Channel condition (available, occupied, …).
    pub condition: u32,
}

/// Enumerate all PCAN channels attached to the system.
///
/// Returns an empty vector if the driver reports no channels. The list
/// includes both available and occupied channels.
pub fn enumerate_channels() -> Result<Vec<ChannelInfo>> {
    let l = error::lib()?;
    let mut count: u32 = 0;
    check_status(unsafe {
        (l.CAN_GetValue)(
            sys::PCAN_NONEBUS,
            sys::PCAN_ATTACHED_CHANNELS_COUNT,
            &mut count as *mut u32 as *mut std::os::raw::c_void,
            core::mem::size_of::<u32>() as u32,
        )
    })?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut raw = vec![sys::TPCANChannelInformation::default(); count as usize];
    let byte_len = count as usize * core::mem::size_of::<sys::TPCANChannelInformation>();
    check_status(unsafe {
        (l.CAN_GetValue)(
            sys::PCAN_NONEBUS,
            sys::PCAN_ATTACHED_CHANNELS,
            raw.as_mut_ptr() as *mut std::os::raw::c_void,
            byte_len as u32,
        )
    })?;
    Ok(raw
        .into_iter()
        .map(|info| {
            let name_len = info
                .device_name
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(info.device_name.len());
            ChannelInfo {
                channel_handle: info.channel_handle,
                device_type: info.device_type,
                controller_number: info.controller_number,
                features: info.device_features,
                device_name: String::from_utf8_lossy(&info.device_name[..name_len]).into_owned(),
                device_id: info.device_id,
                condition: info.channel_condition,
            }
        })
        .collect())
}

/// Get the loaded PCAN-Basic API version as a string.
pub fn get_api_version() -> Result<String> {
    let l = error::lib()?;
    let mut buf = [0u8; sys::MAX_LENGTH_VERSION_STRING];
    check_status(unsafe {
        (l.CAN_GetValue)(
            sys::PCAN_NONEBUS,
            sys::PCAN_API_VERSION,
            buf.as_mut_ptr() as *mut std::os::raw::c_void,
            buf.len() as u32,
        )
    })?;
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..len]).into_owned())
}
