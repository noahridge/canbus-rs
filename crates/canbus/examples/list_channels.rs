//! List the compiled-in interfaces and every channel they can see.
//!
//! ```sh
//! cargo run -p canbus --example list_channels
//! ```

fn main() {
    for interface in canbus::interfaces() {
        match interface.detect() {
            Ok(channels) => {
                println!("{} — {}", interface.name, interface.description);
                if channels.is_empty() {
                    println!("    (no channels)");
                }
                for ch in channels {
                    let serial = ch.serial.as_deref().unwrap_or("-");
                    println!(
                        "    {:<16} {} (serial {serial})",
                        ch.channel, ch.description
                    );
                }
            }
            Err(e) => println!("{} — unavailable: {e}", interface.name),
        }
    }
}
