# canbus

Cross-platform CAN bus access for Rust with runtime-selectable backends —
Kvaser, PEAK PCAN and an in-process virtual bus — in the spirit of
python-can.

```rust
use std::time::Duration;
use canbus::{Bus, BusConfig, CanBus, CanMessage, StandardId};

let mut bus = Bus::open(&BusConfig::new("pcan", "PCAN_USBBUS1").bitrate(500_000))?;
bus.send(&CanMessage::new(StandardId::new(0x123).unwrap(), &[1, 2, 3])?, None)?;
let reply = bus.recv(Some(Duration::from_secs(1)))?;
# Ok::<(), canbus::Error>(())
```

Vendor libraries are loaded at runtime, so the crate builds on every
platform. See the [repository](https://github.com/noahridge/canlib-rs) for
the user guide, examples and the list of interfaces.

License: MIT
