//! In-memory mock channel for tests.

use core::time::Duration;
use std::cell::RefCell;
use std::collections::VecDeque;

use embedded_can::Id;

use crate::bus_params::{Bitrate, BusParams};
use crate::channel::{CanBusControl, CanDiagnostics, CanRead, CanWrite, Filter};
use crate::error::{CanError, Result};
use crate::message::{id_to_raw_and_flag, CanMessage};
use crate::status::{BusStatus, ErrorCounters};

/// A mock CAN channel for unit testing.
pub struct MockChannel {
    on_bus: bool,
    rx_queue: RefCell<VecDeque<CanMessage>>,
    tx_log: RefCell<Vec<CanMessage>>,
}

impl MockChannel {
    pub fn new() -> Self {
        Self {
            on_bus: false,
            rx_queue: RefCell::new(VecDeque::new()),
            tx_log: RefCell::new(Vec::new()),
        }
    }

    /// Enqueue a message that will be returned by the next `read`.
    pub fn push_rx(&self, msg: CanMessage) {
        self.rx_queue.borrow_mut().push_back(msg);
    }

    /// Take all transmitted messages out of the log.
    pub fn take_tx_log(&self) -> Vec<CanMessage> {
        self.tx_log.borrow_mut().drain(..).collect()
    }
}

impl Default for MockChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl CanBusControl for MockChannel {
    fn bus_on(&mut self) -> Result<()> {
        self.on_bus = true;
        Ok(())
    }
    fn bus_off(&mut self) -> Result<()> {
        self.on_bus = false;
        Ok(())
    }
    fn reset_bus(&mut self) -> Result<()> {
        self.on_bus = false;
        Ok(())
    }
    fn is_on_bus(&self) -> bool {
        self.on_bus
    }
    fn set_bitrate(&self, _bitrate: Bitrate) -> Result<()> {
        Ok(())
    }
    fn set_bus_params(&self, _params: &BusParams) -> Result<()> {
        Ok(())
    }
}

impl CanRead for MockChannel {
    fn read(&self) -> Result<CanMessage> {
        self.rx_queue
            .borrow_mut()
            .pop_front()
            .ok_or(CanError::NoMsg)
    }
    fn read_wait(&self, _timeout: Duration) -> Result<CanMessage> {
        self.read()
    }
    fn read_specific(&self, id: Id) -> Result<CanMessage> {
        let (raw, _) = id_to_raw_and_flag(id);
        let mut q = self.rx_queue.borrow_mut();
        match q.iter().position(|m| m.id() == raw) {
            Some(i) => Ok(q.remove(i).unwrap()),
            None => Err(CanError::NoMsg),
        }
    }
    fn read_specific_skip(&self, id: Id) -> Result<CanMessage> {
        self.read_specific(id)
    }
}

impl CanWrite for MockChannel {
    fn write(&self, msg: &CanMessage) -> Result<()> {
        self.tx_log.borrow_mut().push(msg.clone());
        Ok(())
    }
    fn write_wait(&self, msg: &CanMessage, _timeout: Duration) -> Result<()> {
        self.write(msg)
    }
    fn write_sync(&self, _timeout: Duration) -> Result<()> {
        Ok(())
    }
}

impl CanDiagnostics for MockChannel {
    fn set_acceptance_filter(&self, _filter: &Filter) -> Result<()> {
        Ok(())
    }
    fn read_status(&self) -> Result<BusStatus> {
        Ok(BusStatus::empty())
    }
    fn read_error_counters(&self) -> Result<ErrorCounters> {
        Ok(ErrorCounters::default())
    }
    fn flush_rx(&self) -> Result<()> {
        self.rx_queue.borrow_mut().clear();
        Ok(())
    }
    fn flush_tx(&self) -> Result<()> {
        self.tx_log.borrow_mut().clear();
        Ok(())
    }
}

crate::impl_embedded_can!(MockChannel);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::CanChannel;
    use crate::message::MessageFlags;
    use embedded_can::StandardId;

    #[test]
    fn bus_on_off_state() {
        let mut mock = MockChannel::new();
        assert!(!mock.is_on_bus());
        mock.bus_on().unwrap();
        assert!(mock.is_on_bus());
        mock.bus_off().unwrap();
        assert!(!mock.is_on_bus());
    }

    #[test]
    fn write_captures_to_tx_log() {
        let mock = MockChannel::new();
        let msg = CanMessage::new(StandardId::new(0x100).unwrap(), &[1, 2, 3]).unwrap();
        CanWrite::write(&mock, &msg).unwrap();
        let log = mock.take_tx_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].id(), 0x100);
    }

    #[test]
    fn read_returns_enqueued_messages() {
        let mock = MockChannel::new();
        let msg = CanMessage::from_raw(0x200, vec![0xAA], 1, MessageFlags::STD, 1000);
        mock.push_rx(msg);
        let rx = CanRead::read(&mock).unwrap();
        assert_eq!(rx.id(), 0x200);
    }

    #[test]
    fn empty_read_returns_no_msg() {
        let mock = MockChannel::new();
        assert_eq!(CanRead::read(&mock).unwrap_err(), CanError::NoMsg);
    }

    #[test]
    fn dyn_can_channel_acceptance() {
        let mut mock = MockChannel::new();
        let ch: &mut dyn CanChannel = &mut mock;
        ch.bus_on().unwrap();
        assert!(ch.is_on_bus());
    }
}
