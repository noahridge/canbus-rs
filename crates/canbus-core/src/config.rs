//! Bus configuration, vendor-neutral.
//!
//! A [`BusConfig`] names the interface and channel to open and carries the
//! bitrate and mode settings. Backend-specific settings go in
//! [`BusConfig::options`], so the shared types never grow vendor fields.

use std::collections::BTreeMap;

use crate::error::{Error, Result};

/// Explicit bit timing for one CAN phase.
///
/// `tseg1` includes the propagation segment (`prop + phase1`), matching how
/// most controllers and vendor APIs express it. The resulting bitrate is
/// `clock_hz / (brp * (1 + tseg1 + tseg2))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Controller clock frequency in Hz.
    pub clock_hz: u32,
    /// Bitrate prescaler.
    pub brp: u32,
    /// Time segment 1 (propagation + phase 1), in time quanta.
    pub tseg1: u32,
    /// Time segment 2 (phase 2), in time quanta.
    pub tseg2: u32,
    /// Synchronisation jump width, in time quanta.
    pub sjw: u32,
}

impl Timing {
    /// Number of time quanta per bit.
    pub fn quanta_per_bit(&self) -> u32 {
        1 + self.tseg1 + self.tseg2
    }

    /// The bitrate this timing produces, in bit/s (0 if `brp` is 0).
    pub fn bitrate(&self) -> u32 {
        let divisor = self.brp.saturating_mul(self.quanta_per_bit());
        if divisor == 0 {
            0
        } else {
            self.clock_hz / divisor
        }
    }

    /// Sample point as a fraction of the bit time (e.g. `0.8`).
    pub fn sample_point(&self) -> f32 {
        (1 + self.tseg1) as f32 / self.quanta_per_bit() as f32
    }
}

/// CAN FD data-phase settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdConfig {
    /// Data-phase bitrate in bit/s.
    pub data_bitrate: u32,
    /// Explicit data-phase timing. When set it takes precedence over
    /// `data_bitrate`.
    pub data_timing: Option<Timing>,
}

/// Everything needed to open a bus.
///
/// Build one with [`BusConfig::new`] and the builder methods:
///
/// ```
/// use canbus_core::BusConfig;
///
/// let cfg = BusConfig::new("pcan", "PCAN_USBBUS1")
///     .bitrate(500_000)
///     .fd(2_000_000)
///     .option("example_key", "value");
/// assert_eq!(cfg.bitrate, Some(500_000));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BusConfig {
    /// Interface (backend) name, e.g. `"kvaser"`, `"pcan"` or `"virtual"`.
    pub interface: String,
    /// Channel name in the backend's format, e.g. `"0"` or `"PCAN_USBBUS1"`.
    pub channel: String,
    /// Nominal (arbitration-phase) bitrate in bit/s.
    pub bitrate: Option<u32>,
    /// Explicit nominal timing. When set it takes precedence over `bitrate`.
    pub timing: Option<Timing>,
    /// CAN FD settings; `None` opens the bus for classic CAN only.
    pub fd: Option<FdConfig>,
    /// Open in listen-only (silent) mode: no transmission, no ACKs.
    pub listen_only: bool,
    /// Backend-specific options. Each backend documents the keys it reads.
    pub options: BTreeMap<String, String>,
}

impl BusConfig {
    /// Start a configuration for `channel` on `interface`.
    pub fn new(interface: impl Into<String>, channel: impl Into<String>) -> Self {
        Self {
            interface: interface.into(),
            channel: channel.into(),
            bitrate: None,
            timing: None,
            fd: None,
            listen_only: false,
            options: BTreeMap::new(),
        }
    }

    /// Set the nominal bitrate in bit/s.
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// Set explicit nominal timing.
    pub fn timing(mut self, timing: Timing) -> Self {
        self.timing = Some(timing);
        self
    }

    /// Enable CAN FD with the given data-phase bitrate in bit/s.
    pub fn fd(mut self, data_bitrate: u32) -> Self {
        let data_timing = self.fd.and_then(|fd| fd.data_timing);
        self.fd = Some(FdConfig {
            data_bitrate,
            data_timing,
        });
        self
    }

    /// Enable CAN FD with explicit data-phase timing.
    pub fn fd_timing(mut self, timing: Timing) -> Self {
        self.fd = Some(FdConfig {
            data_bitrate: timing.bitrate(),
            data_timing: Some(timing),
        });
        self
    }

    /// Enable or disable listen-only mode.
    pub fn listen_only(mut self, listen_only: bool) -> Self {
        self.listen_only = listen_only;
        self
    }

    /// Set a backend-specific option.
    pub fn option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// The nominal bitrate implied by `timing` or `bitrate`, if any.
    pub fn nominal_bitrate(&self) -> Option<u32> {
        self.timing.map(|t| t.bitrate()).or(self.bitrate)
    }

    /// Read a boolean option (`true/false`, `1/0`, `yes/no`, `on/off`).
    ///
    /// Returns `default` when the key is absent and
    /// [`Error::InvalidConfig`] when the value isn't a boolean.
    pub fn bool_option(&self, key: &str, default: bool) -> Result<bool> {
        match self.options.get(key) {
            None => Ok(default),
            Some(v) => parse_bool(v).ok_or_else(|| {
                Error::InvalidConfig(format!("option `{key}`: expected a boolean, got `{v}`"))
            }),
        }
    }
}

/// Parse a boolean the way config files and environment variables spell it.
pub fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_computes_bitrate_and_sample_point() {
        // 80 MHz / (10 * 16) = 500 kbit/s, sample point 13/16.
        let t = Timing {
            clock_hz: 80_000_000,
            brp: 10,
            tseg1: 12,
            tseg2: 3,
            sjw: 1,
        };
        assert_eq!(t.bitrate(), 500_000);
        assert!((t.sample_point() - 0.8125).abs() < 1e-6);
        assert_eq!(Timing { brp: 0, ..t }.bitrate(), 0);
    }

    #[test]
    fn builder_sets_fields() {
        let cfg = BusConfig::new("virtual", "vcan0")
            .bitrate(250_000)
            .fd(4_000_000)
            .listen_only(true)
            .option("k", "v");
        assert_eq!(cfg.interface, "virtual");
        assert_eq!(cfg.channel, "vcan0");
        assert_eq!(cfg.nominal_bitrate(), Some(250_000));
        assert_eq!(cfg.fd.unwrap().data_bitrate, 4_000_000);
        assert!(cfg.listen_only);
        assert_eq!(cfg.options.get("k").map(String::as_str), Some("v"));
    }

    #[test]
    fn timing_overrides_bitrate() {
        let t = Timing {
            clock_hz: 8_000_000,
            brp: 1,
            tseg1: 12,
            tseg2: 3,
            sjw: 1,
        };
        let cfg = BusConfig::new("x", "y").bitrate(125_000).timing(t);
        assert_eq!(cfg.nominal_bitrate(), Some(500_000));
    }

    #[test]
    fn bool_options_parse() {
        let cfg = BusConfig::new("x", "y")
            .option("a", "Yes")
            .option("b", "maybe");
        assert_eq!(cfg.bool_option("a", false), Ok(true));
        assert_eq!(cfg.bool_option("missing", true), Ok(true));
        assert!(matches!(
            cfg.bool_option("b", false),
            Err(Error::InvalidConfig(_))
        ));
    }
}
