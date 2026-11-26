//! Two virtual buses on one channel, one of them driven from another thread.
//!
//! Runs anywhere — no hardware or vendor driver needed:
//!
//! ```sh
//! cargo run -p canbus --example virtual_loopback
//! ```

use std::thread;
use std::time::Duration;

use canbus::{Brs, Bus, BusConfig, CanBus, CanMessage, Filter, StandardId};

const TIMEOUT: Option<Duration> = Some(Duration::from_secs(1));

fn main() -> canbus::Result<()> {
    let config = BusConfig::new("virtual", "loopback")
        .bitrate(500_000)
        .fd(2_000_000);
    let mut tester = Bus::open(&config)?;
    let ecu = Bus::open(&config)?;

    // A bus is `Send`: hand the "ECU" to its own thread, which answers each
    // request with the payload reversed on ID + 8.
    let responder = thread::spawn(move || -> canbus::Result<()> {
        let mut ecu = ecu;
        ecu.set_filters(&[Filter::Range {
            from: 0x700,
            to: 0x7EF,
            extended: false,
        }])?;
        while let Some(req) = ecu.recv(TIMEOUT)? {
            let mut payload = req.data().to_vec();
            payload.reverse();
            let reply_id = StandardId::new(req.id() as u16 + 8).expect("in range");
            let reply = if req.is_fd() {
                CanMessage::new_fd(reply_id, &payload, Brs::On)?
            } else {
                CanMessage::new(reply_id, &payload)?
            };
            ecu.send(&reply, None)?;
        }
        Ok(())
    });

    let requests = [
        CanMessage::new(StandardId::new(0x7E0).unwrap(), &[0x02, 0x10, 0x01])?,
        CanMessage::new_fd(StandardId::new(0x7E0).unwrap(), &[0xAA; 20], Brs::On)?,
    ];
    for req in &requests {
        tester.send(req, None)?;
        let reply = tester.recv(TIMEOUT)?.expect("ECU replies");
        println!(
            "sent 0x{:03X} [{:2} bytes]  ->  got 0x{:03X} {:02X?}",
            req.id(),
            req.len(),
            reply.id(),
            reply.data()
        );
    }

    // Stop the responder: it exits once its receive times out.
    drop(tester);
    responder.join().expect("responder thread panicked")?;
    println!("PASS");
    Ok(())
}
