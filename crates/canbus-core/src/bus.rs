//! The [`CanBus`] trait every backend implements.

use core::time::Duration;

use crate::error::{Error, Result};
use crate::filter::{Filter, FilterSupport};
use crate::frame::CanMessage;

/// Controller error state, per ISO 11898-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BusState {
    /// Normal operation.
    Active,
    /// An error counter has reached the warning level (96).
    Warning,
    /// An error counter has exceeded 127; the controller sends passive
    /// error flags.
    Passive,
    /// The transmit error counter exceeded 255; the controller is off the bus.
    BusOff,
    /// The backend could not determine the state.
    Unknown,
}

/// Transmit/receive error counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ErrorCounters {
    /// Transmit error counter (TEC).
    pub tx: u32,
    /// Receive error counter (REC).
    pub rx: u32,
}

/// A snapshot of a bus's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusStatus {
    /// Controller error state.
    pub state: BusState,
    /// Error counters, if the backend exposes them.
    pub error_counters: Option<ErrorCounters>,
}

/// What a bus supports, as reported by its backend.
///
/// Construct with [`Capabilities::default`] and the `with_*` methods, so
/// new capabilities can be added without breaking backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Capabilities {
    /// The bus is open in CAN FD mode and can send/receive FD frames.
    pub fd: bool,
    /// The backend can apply at least some filters in hardware.
    pub hardware_filters: bool,
    /// The backend supports listen-only mode.
    pub listen_only: bool,
}

impl Capabilities {
    /// Set [`Capabilities::fd`].
    pub fn with_fd(mut self, fd: bool) -> Self {
        self.fd = fd;
        self
    }

    /// Set [`Capabilities::hardware_filters`].
    pub fn with_hardware_filters(mut self, hardware_filters: bool) -> Self {
        self.hardware_filters = hardware_filters;
        self
    }

    /// Set [`Capabilities::listen_only`].
    pub fn with_listen_only(mut self, listen_only: bool) -> Self {
        self.listen_only = listen_only;
        self
    }
}

/// An open CAN bus.
///
/// This is the contract between applications and backends. Backends return
/// it boxed from [`Interface::open`](crate::Interface); applications usually
/// use it through the `canbus::Bus` facade.
///
/// # Threads
///
/// A bus is [`Send`] (it can be moved to another thread) but not required to
/// be [`Sync`]: methods that touch the device take `&mut self`, so a bus is
/// never entered from two threads at once. This matches vendor rules such as
/// Kvaser's "a handle must not be used concurrently by several threads".
///
/// # Timeouts
///
/// - `None` waits indefinitely.
/// - `Some(Duration::ZERO)` never blocks.
/// - `Some(d)` waits up to `d`.
pub trait CanBus: Send {
    /// Queue `msg` for transmission.
    ///
    /// Returns [`Error::TxQueueFull`] if the queue is full and
    /// `timeout` is zero, or [`Error::Timeout`] if it stays full for the
    /// whole timeout.
    fn send(&mut self, msg: &CanMessage, timeout: Option<Duration>) -> Result<()>;

    /// Receive the next frame, or `Ok(None)` if none arrives within `timeout`.
    ///
    /// Error frames are returned as [`CanMessage::Error`] rather than as `Err`.
    fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<CanMessage>>;

    /// Replace the receive filters. An empty slice accepts every frame.
    ///
    /// See [`FilterSupport`] for what the caller must do with the result.
    fn set_filters(&mut self, filters: &[Filter]) -> Result<FilterSupport>;

    /// Current controller state and error counters.
    fn status(&self) -> Result<BusStatus>;

    /// What this bus supports.
    fn capabilities(&self) -> Capabilities;

    /// The channel name this bus was opened on.
    fn channel_info(&self) -> &str;

    /// Discard frames waiting in the transmit queue.
    fn flush_tx(&mut self) -> Result<()> {
        Err(Error::Unsupported("flush_tx"))
    }

    /// Go off-bus and release the channel. Further sends and receives fail
    /// with [`Error::Shutdown`]. Dropping a bus shuts it down implicitly.
    fn shutdown(&mut self) -> Result<()>;
}

impl<B: CanBus + ?Sized> CanBus for Box<B> {
    fn send(&mut self, msg: &CanMessage, timeout: Option<Duration>) -> Result<()> {
        (**self).send(msg, timeout)
    }
    fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<CanMessage>> {
        (**self).recv(timeout)
    }
    fn set_filters(&mut self, filters: &[Filter]) -> Result<FilterSupport> {
        (**self).set_filters(filters)
    }
    fn status(&self) -> Result<BusStatus> {
        (**self).status()
    }
    fn capabilities(&self) -> Capabilities {
        (**self).capabilities()
    }
    fn channel_info(&self) -> &str {
        (**self).channel_info()
    }
    fn flush_tx(&mut self) -> Result<()> {
        (**self).flush_tx()
    }
    fn shutdown(&mut self) -> Result<()> {
        (**self).shutdown()
    }
}

/// A deadline for loops that poll until a [`CanBus`]-style timeout expires.
///
/// Helper for backends whose native API has no blocking call.
#[derive(Debug, Clone, Copy)]
pub struct Deadline(Option<std::time::Instant>);

impl Deadline {
    /// Start a deadline for `timeout` (see the trait-level timeout rules).
    pub fn after(timeout: Option<Duration>) -> Self {
        Deadline(timeout.map(|t| {
            let now = std::time::Instant::now();
            now.checked_add(t)
                .unwrap_or(now + Duration::from_secs(u32::MAX.into()))
        }))
    }

    /// Time left, or `None` for an infinite deadline.
    pub fn remaining(&self) -> Option<Duration> {
        self.0
            .map(|d| d.saturating_duration_since(std::time::Instant::now()))
    }

    /// True once a finite deadline has passed.
    pub fn expired(&self) -> bool {
        self.remaining().is_some_and(|r| r.is_zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_semantics() {
        assert!(!Deadline::after(None).expired());
        assert!(Deadline::after(None).remaining().is_none());
        assert!(Deadline::after(Some(Duration::ZERO)).expired());
        let d = Deadline::after(Some(Duration::from_secs(60)));
        assert!(!d.expired());
        assert!(d.remaining().unwrap() <= Duration::from_secs(60));
        // Must not panic on overflow.
        assert!(!Deadline::after(Some(Duration::MAX)).expired());
    }

    #[test]
    fn capabilities_builder() {
        let c = Capabilities::default().with_fd(true).with_listen_only(true);
        assert!(c.fd && c.listen_only && !c.hardware_filters);
    }
}
