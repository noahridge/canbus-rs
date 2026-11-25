//! List all available CAN channels.

fn main() {
    let (major, minor) = canbus_kvaser::get_version().expect("Failed to load CANLib");
    println!("CANLib version: {}.{}", major, minor);

    match canbus_kvaser::enumerate_channels() {
        Ok(channels) => {
            println!("Found {} channel(s):", channels.len());
            for ch in &channels {
                println!(
                    "  [{}] {} - {} (serial: {})",
                    ch.index, ch.name, ch.device_description, ch.serial_number
                );
            }
        }
        Err(e) => eprintln!("Error enumerating channels: {}", e),
    }
}
