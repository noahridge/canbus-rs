//! Open a bus in CAN FD mode (500 kbit/s nominal, 2 Mbit/s data) and send a
//! 64-byte frame with bit-rate switching.
//!
//! ```sh
//! cargo run -p canbus --example canfd -- kvaser 0
//! cargo run -p canbus --example canfd -- pcan PCAN_USBBUS1
//! cargo run -p canbus --example canfd -- virtual vcan0
//! ```

use std::time::Duration;

use canbus::{Brs, Bus, BusConfig, CanBus, CanMessage, ExtendedId};

fn main() -> canbus::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [interface, channel] = args.as_slice() else {
        eprintln!("usage: canfd <interface> <channel>");
        std::process::exit(2);
    };

    let config = BusConfig::new(interface, channel)
        .bitrate(500_000)
        .fd(2_000_000);
    let mut bus = Bus::open(&config)?;
    println!(
        "opened {} channel {} (FD: {})",
        bus.interface(),
        bus.channel_info(),
        bus.capabilities().fd
    );

    let payload: Vec<u8> = (0..64).collect();
    let msg = CanMessage::new_fd(ExtendedId::new(0x18DA_F110).unwrap(), &payload, Brs::On)?;
    bus.send(&msg, Some(Duration::from_millis(100)))?;
    println!(
        "sent FD id=0x{:08X} len={} dlc={}",
        msg.id(),
        msg.len(),
        msg.dlc()
    );

    if let Some(rx) = bus.recv(Some(Duration::from_secs(1)))? {
        println!("recv id=0x{:X} len={} fd={}", rx.id(), rx.len(), rx.is_fd());
    }
    Ok(())
}
