//! Backend descriptors.
//!
//! Each backend crate exports a `pub const INTERFACE: Interface` describing
//! how to open its buses and list its channels. The `canbus` facade keeps a
//! list of the compiled-in ones; third-party backends are opened by passing
//! their descriptor directly. No global registry is involved.

use crate::bus::CanBus;
use crate::config::BusConfig;
use crate::error::Result;

/// A channel found by [`Interface::detect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
    /// Interface name, e.g. `"kvaser"`.
    pub interface: &'static str,
    /// Channel name to pass as [`BusConfig::channel`].
    pub channel: String,
    /// Human-readable description (device name, etc.).
    pub description: String,
    /// Device serial number, if known.
    pub serial: Option<String>,
}

impl ChannelInfo {
    /// A [`BusConfig`] that opens this channel.
    pub fn config(&self) -> BusConfig {
        BusConfig::new(self.interface, self.channel.clone())
    }
}

/// Function that opens a bus from a configuration.
pub type OpenFn = fn(&BusConfig) -> Result<Box<dyn CanBus>>;

/// Function that lists available channels.
pub type DetectFn = fn() -> Result<Vec<ChannelInfo>>;

/// Describes one backend.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Interface {
    /// Name used in [`BusConfig::interface`], e.g. `"kvaser"`.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    open: OpenFn,
    detect: DetectFn,
}

impl Interface {
    /// Describe a backend.
    pub const fn new(
        name: &'static str,
        description: &'static str,
        open: OpenFn,
        detect: DetectFn,
    ) -> Self {
        Self {
            name,
            description,
            open,
            detect,
        }
    }

    /// Open a bus. `config.interface` is not checked against [`Self::name`].
    pub fn open(&self, config: &BusConfig) -> Result<Box<dyn CanBus>> {
        (self.open)(config)
    }

    /// List the channels this backend can currently see.
    ///
    /// Fails with [`Error::LibraryNotLoaded`](crate::Error::LibraryNotLoaded)
    /// if the backend's vendor library is not installed.
    pub fn detect(&self) -> Result<Vec<ChannelInfo>> {
        (self.detect)()
    }
}
