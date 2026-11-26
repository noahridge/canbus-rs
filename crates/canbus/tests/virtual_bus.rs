//! End-to-end tests through the `Bus` facade on the virtual backend.

#![cfg(feature = "virtual")]

use std::thread;
use std::time::{Duration, Instant};

use canbus::{
    Brs, Bus, BusConfig, CanBus, CanMessage, Error, ExtendedId, Filter, FilterSupport, StandardId,
};

const T: Option<Duration> = Some(Duration::from_millis(500));

fn pair(channel: &str) -> (Bus, Bus) {
    let cfg = BusConfig::new("virtual", channel).bitrate(500_000);
    (Bus::open(&cfg).unwrap(), Bus::open(&cfg).unwrap())
}

fn std_frame(id: u16) -> CanMessage {
    CanMessage::new(StandardId::new(id).unwrap(), &[id as u8]).unwrap()
}

#[test]
fn open_by_name_and_exchange_frames() {
    let (mut a, mut b) = pair("it-exchange");
    assert_eq!(a.interface(), "virtual");
    assert_eq!(a.channel_info(), "it-exchange");

    a.send(&std_frame(0x42), None).unwrap();
    let rx = b.recv(T).unwrap().expect("frame arrives");
    assert_eq!(rx.id(), 0x42);
    assert_eq!(rx.data(), &[0x42]);
}

#[test]
fn unknown_interface_is_reported() {
    let err = Bus::open(&BusConfig::new("no-such-adapter", "0")).unwrap_err();
    assert_eq!(err, Error::UnknownInterface("no-such-adapter".into()));
}

#[test]
fn recv_timeout_returns_none() {
    let (mut a, _b) = pair("it-timeout");
    let start = Instant::now();
    assert_eq!(a.recv(Some(Duration::from_millis(50))).unwrap(), None);
    assert!(start.elapsed() >= Duration::from_millis(50));
    assert_eq!(a.recv(Some(Duration::ZERO)).unwrap(), None);
}

#[test]
fn software_filters_drop_non_matching_frames() {
    let (mut tx, mut rx) = pair("it-filters");
    let support = rx
        .set_filters(&[
            Filter::standard(0x100),
            Filter::Range {
                from: 0x200,
                to: 0x20F,
                extended: false,
            },
        ])
        .unwrap();
    // The facade guarantees matching frames whatever the backend does.
    assert_eq!(support, FilterSupport::Hardware);

    for id in [0x099, 0x100, 0x150, 0x205, 0x210] {
        tx.send(&std_frame(id), None).unwrap();
    }
    let ext = CanMessage::new(ExtendedId::new(0x100).unwrap(), &[]).unwrap();
    tx.send(&ext, None).unwrap();

    let mut got = Vec::new();
    while let Some(msg) = rx.recv(Some(Duration::from_millis(100))).unwrap() {
        got.push(msg.id());
    }
    assert_eq!(got, vec![0x100, 0x205]);

    // Clearing the filters lets everything through again.
    rx.set_filters(&[]).unwrap();
    tx.send(&std_frame(0x099), None).unwrap();
    assert_eq!(rx.recv(T).unwrap().unwrap().id(), 0x099);
}

#[test]
fn fd_frames_need_an_fd_bus() {
    let fd_frame = CanMessage::new_fd(StandardId::new(0x1).unwrap(), &[7; 48], Brs::On).unwrap();

    let (mut classic, _) = pair("it-fd-classic");
    assert!(matches!(
        classic.send(&fd_frame, None),
        Err(Error::InvalidFrame(_))
    ));

    let cfg = BusConfig::new("virtual", "it-fd")
        .bitrate(500_000)
        .fd(2_000_000);
    let mut a = Bus::open(&cfg).unwrap();
    let mut b = Bus::open(&cfg).unwrap();
    assert!(a.capabilities().fd);
    a.send(&fd_frame, None).unwrap();
    let rx = b.recv(T).unwrap().unwrap();
    assert!(rx.is_fd());
    assert_eq!(rx.len(), 48);
    assert_eq!(rx.dlc(), 14);
}

#[test]
fn bus_can_move_to_another_thread() {
    let (mut a, b) = pair("it-thread");
    let handle = thread::spawn(move || {
        let mut b = b;
        b.recv(Some(Duration::from_secs(2)))
            .unwrap()
            .map(|m| m.id())
    });
    // Give the receiver a moment to start waiting; the frame is queued
    // either way.
    thread::sleep(Duration::from_millis(20));
    a.send(&std_frame(0x7), None).unwrap();
    assert_eq!(handle.join().unwrap(), Some(0x7));
}

#[test]
fn boxed_backend_and_third_party_open_paths() {
    let cfg = BusConfig::new("ignored-by-open-with", "it-open-with");
    let mut a = Bus::open_with(&canbus::virtual_bus::INTERFACE, &cfg).unwrap();
    let raw = canbus::virtual_bus::VirtualBus::open(&cfg).unwrap();
    let mut b = Bus::from_backend("virtual", raw);
    a.send(&std_frame(0x3), None).unwrap();
    assert_eq!(b.recv(T).unwrap().unwrap().id(), 0x3);
}

#[test]
fn detect_lists_open_virtual_channels() {
    let (_a, _b) = pair("it-detect");
    assert!(
        canbus::detect_available()
            .iter()
            .any(|c| c.interface == "virtual" && c.channel == "it-detect")
    );
}

#[test]
fn embedded_can_traits() {
    let (mut a, mut b) = pair("it-embedded");
    embedded_can::blocking::Can::transmit(&mut a, &std_frame(0x55)).unwrap();
    let rx = embedded_can::blocking::Can::receive(&mut b).unwrap();
    assert_eq!(rx.id(), 0x55);
    assert!(matches!(
        embedded_can::nb::Can::receive(&mut b),
        Err(nb::Error::WouldBlock)
    ));
}

#[test]
fn shutdown_stops_io() {
    let (mut a, _b) = pair("it-shutdown");
    a.shutdown().unwrap();
    assert_eq!(a.send(&std_frame(1), None), Err(Error::Shutdown));
}
