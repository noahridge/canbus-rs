//! In-process virtual CAN bus for the `canbus` crate family.
//!
//! Every bus opened on the same channel name within a process is connected:
//! a frame sent on one is received by all the others. No hardware or driver
//! is needed, so it works on every platform and is the backend to use in
//! tests and examples.
//!
//! Interface name: `"virtual"`. Channel: any string, e.g. `"vcan0"`.
//!
//! Options:
//! - `receive_own_messages` (bool, default `false`): also deliver a bus's own
//!   frames back to it, flagged as echoes.
//!
//! The bus is classic-CAN only unless the config enables CAN FD
//! ([`BusConfig::fd`]); sending an FD frame on a classic bus fails, as it
//! would on hardware. Bitrates are accepted and ignored.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use canbus_core::{
    BusConfig, BusState, BusStatus, CanBus, CanMessage, Capabilities, ChannelInfo, Error, Filter,
    FilterSupport, Interface, MessageFlags, Result,
};

/// Descriptor for the virtual backend.
pub const INTERFACE: Interface = Interface::new(
    "virtual",
    "In-process virtual bus (no hardware)",
    open_boxed,
    detect,
);

fn open_boxed(config: &BusConfig) -> Result<Box<dyn CanBus>> {
    Ok(Box::new(VirtualBus::open(config)?))
}

/// Subscribers per channel name.
type Hub = HashMap<String, Vec<(u64, Sender<CanMessage>)>>;

fn hub() -> MutexGuard<'static, Hub> {
    static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();
    HUB.get_or_init(Default::default)
        .lock()
        // A panic while holding the lock can't leave the map inconsistent.
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Shared epoch so timestamps from different buses are comparable.
fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// List the channel names that currently have at least one open bus.
pub fn detect() -> Result<Vec<ChannelInfo>> {
    let mut names: Vec<String> = hub()
        .iter()
        .filter(|(_, subs)| !subs.is_empty())
        .map(|(name, _)| name.clone())
        .collect();
    names.sort();
    Ok(names
        .into_iter()
        .map(|channel| ChannelInfo {
            interface: INTERFACE.name,
            channel,
            description: "virtual bus".into(),
            serial: None,
        })
        .collect())
}

/// One endpoint on a virtual channel.
#[derive(Debug)]
pub struct VirtualBus {
    channel: String,
    id: u64,
    rx: Receiver<CanMessage>,
    fd: bool,
    listen_only: bool,
    receive_own: bool,
    open: bool,
}

impl VirtualBus {
    /// Join the virtual channel named in `config.channel`.
    pub fn open(config: &BusConfig) -> Result<Self> {
        if config.channel.is_empty() {
            return Err(Error::InvalidConfig("virtual channel name is empty".into()));
        }
        let receive_own = config.bool_option("receive_own_messages", false)?;
        let (tx, rx) = mpsc::channel();
        let id = next_id();
        hub()
            .entry(config.channel.clone())
            .or_default()
            .push((id, tx));
        Ok(Self {
            channel: config.channel.clone(),
            id,
            rx,
            fd: config.fd.is_some(),
            listen_only: config.listen_only,
            receive_own,
            open: true,
        })
    }

    fn ensure_open(&self) -> Result<()> {
        if self.open {
            Ok(())
        } else {
            Err(Error::Shutdown)
        }
    }

    fn leave(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        let mut hub = hub();
        if let Some(subs) = hub.get_mut(&self.channel) {
            subs.retain(|(id, _)| *id != self.id);
            if subs.is_empty() {
                hub.remove(&self.channel);
            }
        }
    }
}

impl CanBus for VirtualBus {
    fn send(&mut self, msg: &CanMessage, _timeout: Option<Duration>) -> Result<()> {
        self.ensure_open()?;
        if self.listen_only {
            return Err(Error::Unsupported("send on a listen-only bus"));
        }
        if msg.is_fd() && !self.fd {
            return Err(Error::InvalidFrame("CAN FD frame on a classic CAN bus"));
        }
        let stamped = msg.clone().with_timestamp(Some(epoch().elapsed()));
        let mut hub = hub();
        if let Some(subs) = hub.get_mut(&self.channel) {
            // Drop subscribers whose bus has gone away without unregistering.
            subs.retain(|(id, tx)| {
                let frame = if *id == self.id {
                    if !self.receive_own {
                        return true;
                    }
                    echo(&stamped)
                } else {
                    stamped.clone()
                };
                tx.send(frame).is_ok()
            });
        }
        Ok(())
    }

    fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<CanMessage>> {
        self.ensure_open()?;
        loop {
            let msg = match timeout {
                Some(t) if t.is_zero() => match self.rx.try_recv() {
                    Ok(m) => m,
                    Err(TryRecvError::Empty) => return Ok(None),
                    Err(TryRecvError::Disconnected) => return Err(Error::Shutdown),
                },
                Some(t) => match self.rx.recv_timeout(t) {
                    Ok(m) => m,
                    Err(RecvTimeoutError::Timeout) => return Ok(None),
                    Err(RecvTimeoutError::Disconnected) => return Err(Error::Shutdown),
                },
                None => self.rx.recv().map_err(|_| Error::Shutdown)?,
            };
            // A classic-CAN node can't decode FD frames; real hardware
            // reports them as errors, here they are simply not delivered.
            if msg.is_fd() && !self.fd {
                continue;
            }
            return Ok(Some(msg));
        }
    }

    fn set_filters(&mut self, _filters: &[Filter]) -> Result<FilterSupport> {
        self.ensure_open()?;
        Ok(FilterSupport::Software)
    }

    fn status(&self) -> Result<BusStatus> {
        self.ensure_open()?;
        Ok(BusStatus {
            state: BusState::Active,
            error_counters: None,
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
            .with_fd(self.fd)
            .with_listen_only(true)
    }

    fn channel_info(&self) -> &str {
        &self.channel
    }

    fn flush_tx(&mut self) -> Result<()> {
        // Frames are delivered synchronously, so nothing is ever queued.
        self.ensure_open()
    }

    fn shutdown(&mut self) -> Result<()> {
        self.leave();
        Ok(())
    }
}

impl Drop for VirtualBus {
    fn drop(&mut self) {
        self.leave();
    }
}

fn echo(msg: &CanMessage) -> CanMessage {
    CanMessage::from_raw(
        msg.id(),
        msg.data(),
        msg.len() as u8,
        msg.flags() | MessageFlags::ECHO,
        msg.timestamp(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use canbus_core::{Brs, StandardId};

    const T: Option<Duration> = Some(Duration::from_millis(200));

    fn frame(id: u16, data: &[u8]) -> CanMessage {
        CanMessage::new(StandardId::new(id).unwrap(), data).unwrap()
    }

    #[test]
    fn frames_reach_other_buses_on_the_same_channel_only() {
        let cfg = BusConfig::new("virtual", "unit-a");
        let mut a = VirtualBus::open(&cfg).unwrap();
        let mut b = VirtualBus::open(&cfg).unwrap();
        let mut other = VirtualBus::open(&BusConfig::new("virtual", "unit-b")).unwrap();

        a.send(&frame(0x10, &[1, 2]), None).unwrap();
        let rx = b.recv(T).unwrap().expect("b receives");
        assert_eq!(rx.id(), 0x10);
        assert_eq!(rx.data(), &[1, 2]);
        assert!(rx.timestamp().is_some());
        assert_eq!(a.recv(Some(Duration::ZERO)).unwrap(), None, "no self-echo");
        assert_eq!(other.recv(Some(Duration::ZERO)).unwrap(), None);
    }

    #[test]
    fn receive_own_messages_delivers_echo() {
        let cfg = BusConfig::new("virtual", "unit-echo").option("receive_own_messages", "true");
        let mut a = VirtualBus::open(&cfg).unwrap();
        a.send(&frame(0x1, &[]), None).unwrap();
        let rx = a.recv(T).unwrap().unwrap();
        assert!(rx.is_echo());
    }

    #[test]
    fn fd_requires_fd_config() {
        let fd_frame = CanMessage::new_fd(StandardId::new(1).unwrap(), &[0; 12], Brs::On).unwrap();
        let mut classic = VirtualBus::open(&BusConfig::new("virtual", "unit-fd")).unwrap();
        assert!(matches!(
            classic.send(&fd_frame, None),
            Err(Error::InvalidFrame(_))
        ));

        let fd_cfg = BusConfig::new("virtual", "unit-fd").fd(2_000_000);
        let mut fd_a = VirtualBus::open(&fd_cfg).unwrap();
        let mut fd_b = VirtualBus::open(&fd_cfg).unwrap();
        fd_a.send(&fd_frame, None).unwrap();
        assert_eq!(fd_b.recv(T).unwrap().unwrap().len(), 12);
        assert_eq!(
            classic.recv(Some(Duration::ZERO)).unwrap(),
            None,
            "classic node doesn't see FD frames"
        );
    }

    #[test]
    fn shutdown_unregisters_and_rejects_io() {
        let mut a = VirtualBus::open(&BusConfig::new("virtual", "unit-shutdown")).unwrap();
        assert!(
            detect()
                .unwrap()
                .iter()
                .any(|c| c.channel == "unit-shutdown")
        );
        a.shutdown().unwrap();
        assert!(
            !detect()
                .unwrap()
                .iter()
                .any(|c| c.channel == "unit-shutdown")
        );
        assert_eq!(a.send(&frame(1, &[]), None), Err(Error::Shutdown));
        assert_eq!(a.recv(Some(Duration::ZERO)), Err(Error::Shutdown));
    }

    #[test]
    fn listen_only_cannot_send() {
        let mut a =
            VirtualBus::open(&BusConfig::new("virtual", "unit-lo").listen_only(true)).unwrap();
        assert!(matches!(
            a.send(&frame(1, &[]), None),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn timeout_returns_none() {
        let mut a = VirtualBus::open(&BusConfig::new("virtual", "unit-timeout")).unwrap();
        let start = Instant::now();
        assert_eq!(a.recv(Some(Duration::from_millis(30))).unwrap(), None);
        assert!(start.elapsed() >= Duration::from_millis(30));
    }
}
