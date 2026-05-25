//! Bitrate, timing, and driver configuration types.
//!
//! These are vendor-agnostic. Each backend translates the values into its
//! native form (Kvaser BTR constants, PCAN BTR0/BTR1 bytes, etc.).

/// Predefined CAN bitrates for classic CAN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitrate {
    /// 1 Mbit/s
    Rate1M,
    /// 500 kbit/s
    Rate500K,
    /// 250 kbit/s
    Rate250K,
    /// 125 kbit/s
    Rate125K,
    /// 100 kbit/s
    Rate100K,
    /// 83.333 kbit/s
    Rate83K,
    /// 62.5 kbit/s
    Rate62K,
    /// 50 kbit/s
    Rate50K,
    /// 10 kbit/s
    Rate10K,
}

/// Predefined CAN FD data-phase bitrates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdBitrate {
    /// 500 kbit/s, 80% sample point
    Rate500K80P,
    /// 1 Mbit/s, 80% sample point
    Rate1M80P,
    /// 2 Mbit/s, 80% sample point
    Rate2M80P,
    /// 4 Mbit/s, 80% sample point
    Rate4M80P,
    /// 8 Mbit/s, 60% sample point
    Rate8M60P,
    /// 8 Mbit/s, 70% sample point
    Rate8M70P,
    /// 8 Mbit/s, 80% sample point
    Rate8M80P,
}

/// Custom bus timing parameters for classic CAN.
#[derive(Debug, Clone, Copy)]
pub struct BusParams {
    freq: i64,
    tseg1: u32,
    tseg2: u32,
    sjw: u32,
    no_samp: u32,
    sync_mode: u32,
}

impl BusParams {
    /// Create new bus parameters.
    pub fn new(freq: i64, tseg1: u32, tseg2: u32, sjw: u32, no_samp: u32, sync_mode: u32) -> Self {
        Self { freq, tseg1, tseg2, sjw, no_samp, sync_mode }
    }

    pub fn freq(&self) -> i64 { self.freq }
    pub fn tseg1(&self) -> u32 { self.tseg1 }
    pub fn tseg2(&self) -> u32 { self.tseg2 }
    pub fn sjw(&self) -> u32 { self.sjw }
    pub fn no_samp(&self) -> u32 { self.no_samp }
    pub fn sync_mode(&self) -> u32 { self.sync_mode }
}

/// Bus timing parameters using time quanta.
#[derive(Debug, Clone, Copy)]
pub struct BusParamsTq {
    tq: i32,
    phase1: i32,
    phase2: i32,
    sjw: i32,
    prop: i32,
    prescaler: i32,
}

impl BusParamsTq {
    /// Create new time-quanta bus parameters.
    pub fn new(tq: i32, phase1: i32, phase2: i32, sjw: i32, prop: i32, prescaler: i32) -> Self {
        Self { tq, phase1, phase2, sjw, prop, prescaler }
    }

    pub fn tq(&self) -> i32 { self.tq }
    pub fn phase1(&self) -> i32 { self.phase1 }
    pub fn phase2(&self) -> i32 { self.phase2 }
    pub fn sjw(&self) -> i32 { self.sjw }
    pub fn prop(&self) -> i32 { self.prop }
    pub fn prescaler(&self) -> i32 { self.prescaler }
}

/// Driver output mode for the CAN transceiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverType {
    /// Normal mode.
    Normal,
    /// Silent / listen-only mode.
    Silent,
    /// Self-reception mode.
    SelfReception,
    /// Off.
    Off,
}
