//! Bus status, error counters, and statistics types.

bitflags::bitflags! {
    /// CAN bus status flags.
    ///
    /// Bit values are crate-defined; backends translate from native status
    /// codes (Kvaser `canSTAT_*`, PCAN bus-error bits, etc.).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BusStatus: u64 {
        /// The bus is in error passive state.
        const ERROR_PASSIVE = 0x0000_0001;
        /// The bus is off (too many errors).
        const BUS_OFF = 0x0000_0002;
        /// Error warning level reached.
        const ERROR_WARNING = 0x0000_0004;
        /// The bus is error active (normal operation).
        const ERROR_ACTIVE = 0x0000_0008;
        /// There are messages waiting to be transmitted.
        const TX_PENDING = 0x0000_0010;
        /// There are messages in the receive queue.
        const RX_PENDING = 0x0000_0020;
        /// A receive buffer overrun has occurred.
        const OVERRUN = 0x0000_0040;
    }
}

/// CAN error counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct ErrorCounters {
    /// Transmit error counter.
    pub tx_errors: u32,
    /// Receive error counter.
    pub rx_errors: u32,
    /// Overrun error counter.
    pub overrun_errors: u32,
}

/// CAN bus statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct BusStatistics {
    /// Number of standard data frames.
    pub std_data: u64,
    /// Number of standard remote frames.
    pub std_remote: u64,
    /// Number of extended data frames.
    pub ext_data: u64,
    /// Number of extended remote frames.
    pub ext_remote: u64,
    /// Number of error frames.
    pub err_frames: u64,
    /// Bus load (0-10000, representing 0.00%-100.00%).
    pub bus_load: u64,
    /// Number of overruns.
    pub overruns: u64,
}

impl BusStatistics {
    /// Bus load as a percentage (0.0 - 100.0).
    pub fn bus_load_percent(&self) -> f64 {
        self.bus_load as f64 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_load_percent_computes_correctly() {
        let stats = BusStatistics { bus_load: 5000, ..Default::default() };
        assert!((stats.bus_load_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bus_status_flag_composition() {
        let s = BusStatus::ERROR_ACTIVE | BusStatus::TX_PENDING;
        assert!(s.contains(BusStatus::ERROR_ACTIVE));
        assert!(s.contains(BusStatus::TX_PENDING));
        assert!(!s.contains(BusStatus::BUS_OFF));
    }
}
