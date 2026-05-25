//! Send a CAN FD frame on PCAN USBBUS1 at 500K/2M.

use pcan::{Brs, CanMessage, FdBitrate, PcanBus, PcanChannel, PcanFdBitrate, StandardId};

fn main() -> pcan::Result<()> {
    let mut ch = PcanChannel::open(PcanBus::Usb(1))?;
    ch.set_fd_bitrate(PcanFdBitrate::from_preset(FdBitrate::Rate2M80P)?)?;
    ch.bus_on()?;

    let id = StandardId::new(0x456).expect("valid 11-bit id");
    let msg = CanMessage::new_fd(id, &[0u8; 64], Brs::On)?;
    ch.write(&msg)?;
    println!("sent FD id=0x{:X} dlc={} brs=on", msg.id(), msg.dlc());

    Ok(())
}
