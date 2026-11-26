//! Send one frame and wait up to two seconds for any reply.
//!
//! Works with every interface. Pass the interface and channel on the command
//! line, or set the python-can style `CAN_*` environment variables:
//!
//! ```sh
//! cargo run -p canbus --example send_receive -- kvaser 0 500000
//! cargo run -p canbus --example send_receive -- pcan PCAN_USBBUS1 500000
//! CAN_INTERFACE=pcan CAN_CHANNEL=PCAN_USBBUS1 CAN_BITRATE=500000 \
//!     cargo run -p canbus --example send_receive
//! ```

use std::time::Duration;

use canbus::{Bus, BusConfig, CanBus, CanMessage, StandardId};

fn main() -> canbus::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut bus = match args.as_slice() {
        [] => Bus::from_env()?,
        [interface, channel, rest @ ..] => {
            let bitrate = rest
                .first()
                .map_or(Ok(500_000), |b| b.parse())
                .map_err(|_| canbus::Error::InvalidConfig("bitrate must be a number".into()))?;
            Bus::open(&BusConfig::new(interface, channel).bitrate(bitrate))?
        }
        _ => {
            eprintln!("usage: send_receive [<interface> <channel> [bitrate]]");
            std::process::exit(2);
        }
    };
    println!("opened {} channel {}", bus.interface(), bus.channel_info());

    let msg = CanMessage::new(StandardId::new(0x123).unwrap(), &[0xDE, 0xAD, 0xBE, 0xEF])?;
    bus.send(&msg, Some(Duration::from_millis(100)))?;
    println!("sent id=0x{:03X} data={:02X?}", msg.id(), msg.data());

    match bus.recv(Some(Duration::from_secs(2)))? {
        Some(rx) => println!(
            "recv id=0x{:X} data={:02X?} at {:?}",
            rx.id(),
            rx.data(),
            rx.timestamp()
        ),
        None => println!("no reply within 2 s"),
    }
    Ok(())
}
