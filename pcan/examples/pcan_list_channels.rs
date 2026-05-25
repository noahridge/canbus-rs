//! List all PCAN channels attached to the system.

use pcan::enumerate_channels;

fn main() -> pcan::Result<()> {
    println!("PCAN-Basic API version: {}", pcan::get_api_version()?);
    let channels = enumerate_channels()?;
    if channels.is_empty() {
        println!("No PCAN channels detected.");
        return Ok(());
    }
    println!("Found {} channel(s):", channels.len());
    for c in channels {
        println!(
            "  handle=0x{:03X} type={} ctrl={} name={:?} id={} cond=0x{:X}",
            c.channel_handle, c.device_type, c.controller_number, c.device_name, c.device_id, c.condition,
        );
    }
    Ok(())
}
