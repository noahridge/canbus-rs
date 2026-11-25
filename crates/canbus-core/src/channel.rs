//! CAN channel traits shared by all backends.
//!
//! Backends provide concrete channel types (`canbus_kvaser::Channel`,
//! `canbus_pcan::PcanChannel`) that implement these traits. Generic code can be
//! written against [`CanChannel`] or any of the sub-traits.

use core::time::Duration;
use embedded_can::Id;

use crate::bus_params::{Bitrate, BusParams};
use crate::error::Result;
use crate::message::CanMessage;
use crate::status::{BusStatus, ErrorCounters};

/// Bus control operations: on/off, bitrate, and bus parameters.
pub trait CanBusControl {
    fn bus_on(&mut self) -> Result<()>;
    fn bus_off(&mut self) -> Result<()>;
    fn reset_bus(&mut self) -> Result<()>;
    fn is_on_bus(&self) -> bool;
    fn set_bitrate(&self, bitrate: Bitrate) -> Result<()>;
    fn set_bus_params(&self, params: &BusParams) -> Result<()>;
}

/// Read operations on a CAN channel.
pub trait CanRead {
    fn read(&self) -> Result<CanMessage>;
    fn read_wait(&self, timeout: Duration) -> Result<CanMessage>;
    fn read_specific(&self, id: Id) -> Result<CanMessage>;
    fn read_specific_skip(&self, id: Id) -> Result<CanMessage>;
}

/// Write operations on a CAN channel.
pub trait CanWrite {
    fn write(&self, msg: &CanMessage) -> Result<()>;
    fn write_wait(&self, msg: &CanMessage, timeout: Duration) -> Result<()>;
    fn write_sync(&self, timeout: Duration) -> Result<()>;
}

/// An acceptance filter description.
///
/// Backends accept either form and translate as needed; if a backend cannot
/// represent a given filter natively (e.g. non-contiguous masks on a
/// range-only backend), the call returns [`crate::error::CanError::NotImplemented`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Code/mask filter (Kvaser-native).
    CodeMask {
        code: u32,
        mask: u32,
        extended: bool,
    },
    /// Inclusive range filter (PCAN-native).
    Range { from: u32, to: u32, extended: bool },
}

/// Diagnostics: filters, status, error counters, and queue flushing.
pub trait CanDiagnostics {
    /// Configure an acceptance filter. The backend translates into its
    /// native form; non-representable filters return `NotImplemented`.
    fn set_acceptance_filter(&self, filter: &Filter) -> Result<()>;
    fn read_status(&self) -> Result<BusStatus>;
    fn read_error_counters(&self) -> Result<ErrorCounters>;
    fn flush_rx(&self) -> Result<()>;
    fn flush_tx(&self) -> Result<()>;
}

/// Combined trait encompassing all CAN channel operations.
pub trait CanChannel: CanBusControl + CanRead + CanWrite + CanDiagnostics {}

impl<T: CanBusControl + CanRead + CanWrite + CanDiagnostics> CanChannel for T {}

/// Generate trait implementations for all four CAN sub-traits by delegating
/// to inherent methods of the same name on `$type`.
///
/// Backends use this macro after defining the inherent methods to avoid
/// the boilerplate of trait `impl` blocks.
#[macro_export]
macro_rules! impl_can_channel {
    ($type:ty) => {
        impl $crate::channel::CanBusControl for $type {
            fn bus_on(&mut self) -> $crate::error::Result<()> {
                <$type>::bus_on(self)
            }
            fn bus_off(&mut self) -> $crate::error::Result<()> {
                <$type>::bus_off(self)
            }
            fn reset_bus(&mut self) -> $crate::error::Result<()> {
                <$type>::reset_bus(self)
            }
            fn is_on_bus(&self) -> bool {
                <$type>::is_on_bus(self)
            }
            fn set_bitrate(
                &self,
                bitrate: $crate::bus_params::Bitrate,
            ) -> $crate::error::Result<()> {
                <$type>::set_bitrate(self, bitrate)
            }
            fn set_bus_params(
                &self,
                params: &$crate::bus_params::BusParams,
            ) -> $crate::error::Result<()> {
                <$type>::set_bus_params(self, params)
            }
        }

        impl $crate::channel::CanRead for $type {
            fn read(&self) -> $crate::error::Result<$crate::message::CanMessage> {
                <$type>::read(self)
            }
            fn read_wait(
                &self,
                timeout: core::time::Duration,
            ) -> $crate::error::Result<$crate::message::CanMessage> {
                <$type>::read_wait(self, timeout)
            }
            fn read_specific(
                &self,
                id: $crate::embedded_can::Id,
            ) -> $crate::error::Result<$crate::message::CanMessage> {
                <$type>::read_specific(self, id)
            }
            fn read_specific_skip(
                &self,
                id: $crate::embedded_can::Id,
            ) -> $crate::error::Result<$crate::message::CanMessage> {
                <$type>::read_specific_skip(self, id)
            }
        }

        impl $crate::channel::CanWrite for $type {
            fn write(&self, msg: &$crate::message::CanMessage) -> $crate::error::Result<()> {
                <$type>::write(self, msg)
            }
            fn write_wait(
                &self,
                msg: &$crate::message::CanMessage,
                timeout: core::time::Duration,
            ) -> $crate::error::Result<()> {
                <$type>::write_wait(self, msg, timeout)
            }
            fn write_sync(&self, timeout: core::time::Duration) -> $crate::error::Result<()> {
                <$type>::write_sync(self, timeout)
            }
        }

        impl $crate::channel::CanDiagnostics for $type {
            fn set_acceptance_filter(
                &self,
                filter: &$crate::channel::Filter,
            ) -> $crate::error::Result<()> {
                <$type>::set_acceptance_filter(self, filter)
            }
            fn read_status(&self) -> $crate::error::Result<$crate::status::BusStatus> {
                <$type>::read_status(self)
            }
            fn read_error_counters(&self) -> $crate::error::Result<$crate::status::ErrorCounters> {
                <$type>::read_error_counters(self)
            }
            fn flush_rx(&self) -> $crate::error::Result<()> {
                <$type>::flush_rx(self)
            }
            fn flush_tx(&self) -> $crate::error::Result<()> {
                <$type>::flush_tx(self)
            }
        }
    };
}
