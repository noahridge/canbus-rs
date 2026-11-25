//! Send a CAN frame on PCAN USBBUS1, then wait up to 2 s for a reply.

use canbus_pcan::{Bitrate, CanError, CanMessage, PcanBus, PcanChannel, StandardId};
use std::time::Duration;

fn main() -> canbus_pcan::Result<()> {
    let mut ch = PcanChannel::open(PcanBus::Usb(1))?;
    ch.set_bitrate(Bitrate::Rate500K)?;
    ch.bus_on()?;

    let id = StandardId::new(0x123).expect("valid 11-bit id");
    let msg = CanMessage::new(id, &[0xDE, 0xAD, 0xBE, 0xEF])?;
    ch.write(&msg)?;
    println!("sent id=0x{:X} data={:02X?}", msg.id(), msg.data());

    match ch.read_wait(Duration::from_secs(2)) {
        Ok(rx) => println!("recv id=0x{:X} data={:02X?}", rx.id(), rx.data()),
        Err(CanError::Timeout) => println!("no reply"),
        Err(e) => return Err(e),
    }

    Ok(())
}
