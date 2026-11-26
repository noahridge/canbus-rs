//! Hardware smoke tests. Ignored by default; run on a machine with the
//! relevant driver installed:
//!
//! ```sh
//! # Kvaser virtual channels 0 and 1 (installed with the Kvaser driver):
//! cargo test -p canbus --test hardware kvaser -- --ignored
//! # Two PCAN-USB channels wired together (defaults PCAN_USBBUS1/2):
//! CANBUS_PCAN_A=PCAN_USBBUS1 CANBUS_PCAN_B=PCAN_USBBUS2 \
//!     cargo test -p canbus --test hardware pcan -- --ignored
//! ```

use std::thread;
use std::time::Duration;

use canbus::{Brs, Bus, BusConfig, CanBus, CanMessage, StandardId};

const T: Option<Duration> = Some(Duration::from_secs(2));

fn exchange(a: BusConfig, b: BusConfig) {
    let mut a = Bus::open(&a).expect("open first channel");
    let mut b = Bus::open(&b).expect("open second channel");
    let fd = a.capabilities().fd && b.capabilities().fd;

    let classic = CanMessage::new(StandardId::new(0x123).unwrap(), &[1, 2, 3, 4]).unwrap();
    a.send(&classic, T).unwrap();
    let rx = b.recv(T).unwrap().expect("classic frame arrives");
    assert_eq!((rx.id(), rx.data()), (0x123, &[1u8, 2, 3, 4][..]));
    assert!(rx.timestamp().is_some());

    if fd {
        let payload: Vec<u8> = (0..64).collect();
        let msg = CanMessage::new_fd(StandardId::new(0x456).unwrap(), &payload, Brs::On).unwrap();
        a.send(&msg, T).unwrap();
        let rx = b.recv(T).unwrap().expect("FD frame arrives");
        assert!(rx.is_fd());
        assert_eq!(rx.len(), 64, "FD length must be reported in bytes");
        assert_eq!(rx.data(), &payload[..]);
    }

    // Open on this thread, use on another: the CanBus Send contract.
    let worker = thread::spawn(move || {
        let reply = CanMessage::new(StandardId::new(0x321).unwrap(), &[9]).unwrap();
        b.send(&reply, T).unwrap();
        b
    });
    let rx = a.recv(T).unwrap().expect("reply from worker thread");
    assert_eq!(rx.id(), 0x321);
    drop(worker.join().unwrap());
}

#[test]
#[ignore = "needs the Kvaser driver with virtual channels"]
fn kvaser_virtual_channels() {
    let cfg = |ch| BusConfig::new("kvaser", ch).bitrate(500_000).fd(2_000_000);
    exchange(cfg("0"), cfg("1"));
}

#[test]
#[ignore = "needs two connected PCAN channels"]
fn pcan_channels() {
    let a = std::env::var("CANBUS_PCAN_A").unwrap_or_else(|_| "PCAN_USBBUS1".into());
    let b = std::env::var("CANBUS_PCAN_B").unwrap_or_else(|_| "PCAN_USBBUS2".into());
    let cfg = |ch: String| BusConfig::new("pcan", ch).bitrate(500_000);
    exchange(cfg(a), cfg(b));
}
