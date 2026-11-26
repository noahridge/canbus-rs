# canbus

Cross-platform CAN bus access for Rust, with backends selected at runtime —
in the spirit of Python's [python-can](https://python-can.readthedocs.io/).
Write your code once against one API and run it on Kvaser, PEAK PCAN or an
in-process virtual bus.

```rust
use std::time::Duration;
use canbus::{Bus, BusConfig, CanBus, CanMessage, StandardId};

fn main() -> canbus::Result<()> {
    // Or "pcan" / "PCAN_USBBUS1", or "virtual" / "vcan0" — same code.
    let mut bus = Bus::open(&BusConfig::new("kvaser", "0").bitrate(500_000))?;

    let id = StandardId::new(0x123).unwrap();
    bus.send(&CanMessage::new(id, &[0xDE, 0xAD, 0xBE, 0xEF])?, None)?;

    if let Some(rx) = bus.recv(Some(Duration::from_secs(1)))? {
        println!("id=0x{:X} data={:02X?}", rx.id(), rx.data());
    }
    Ok(())
}
```

## Features

- **One API for every adapter** — `Bus` / `CanBus`: send, receive with
  timeout, filters, bus status, CAN FD.
- **Runtime backend selection** — by name, from the command line, or from the
  same `CAN_INTERFACE` / `CAN_CHANNEL` / `CAN_BITRATE` environment variables
  python-can uses (`Bus::from_env()`).
- **No build-time vendor dependencies** — vendor libraries are loaded when a
  bus is opened, so the crate builds everywhere and missing drivers are a
  clear runtime error.
- **Virtual bus** for tests and CI on any OS, no hardware needed.
- **Thread-friendly** — every bus is `Send`; move it to a worker thread.
- **Software filtering fallback** — filters behave the same on every backend,
  even when the hardware can only approximate them.
- **`embedded-can`** — `Bus` implements `embedded_can::blocking::Can` and
  `embedded_can::nb::Can`.
- **Extensible** — third-party backends implement `canbus_core::CanBus` and
  plug in with `Bus::open_with`.

## Interfaces

| Interface | Channel names | Platforms | Requires |
|---|---|---|---|
| `kvaser` | `0`, `1`, … (CANlib channel index) | Windows, Linux | [Kvaser drivers / CANlib SDK](https://kvaser.com/developer/canlib-sdk/) (Linux: [linuxcan](https://www.kvaser.com/linux-drivers-and-sdk-2/)) |
| `pcan` | `PCAN_USBBUS1`, `PCAN_PCIBUS1`, `PCAN_LANBUS1`, … | Windows, Linux | [PEAK PCAN-Basic](https://www.peak-system.com/PCAN-Basic.239.0.html) |
| `virtual` | any name, e.g. `vcan0` | all | nothing |

All three are enabled by default (Cargo features `kvaser`, `pcan`,
`virtual`). If a vendor library is installed somewhere non-standard, set
`CANLIB_SDK_DIR` (Kvaser) or `PCAN_SDK_DIR` (PEAK).

## Install

```toml
[dependencies]
canbus = "0.1"
```

To trim backends: `canbus = { version = "0.1", default-features = false, features = ["pcan"] }`.

## Examples

```sh
cargo run -p canbus --example list_channels                  # every interface and channel found
cargo run -p canbus --example virtual_loopback               # runs anywhere, no hardware
cargo run -p canbus --example send_receive -- kvaser 0 500000
cargo run -p canbus --example canfd -- pcan PCAN_USBBUS1
CAN_INTERFACE=pcan CAN_CHANNEL=PCAN_USBBUS1 CAN_BITRATE=500000 \
    cargo run -p canbus --example send_receive
```

Hardware smoke tests (ignored by default):
`cargo test -p canbus --test hardware -- --ignored`.

## Crates

| Crate | Purpose |
|---|---|
| [`canbus`](crates/canbus) | What applications depend on: `Bus`, runtime backend selection, env config |
| [`canbus-core`](crates/canbus-core) | The stable contract: `CanMessage`, `BusConfig`, `CanBus`, `Interface`, `Error` |
| [`canbus-kvaser`](crates/canbus-kvaser) | Kvaser CANlib backend |
| [`canbus-pcan`](crates/canbus-pcan) | PEAK PCAN-Basic backend |
| [`canbus-virtual`](crates/canbus-virtual) | In-process virtual bus |

See [USER_GUIDE.md](USER_GUIDE.md) for the full guide and
[ARCHITECTURE.md](ARCHITECTURE.md) for how the pieces fit together.

## Troubleshooting

- **`vendor library could not be loaded`** — the vendor driver/SDK isn't
  installed, or its library isn't on the search path. Install it, or set
  `CANLIB_SDK_DIR` / `PCAN_SDK_DIR`. On macOS only `virtual` is available.
- **No Kvaser channels** — no hardware connected and the Kvaser virtual
  driver isn't installed (it ships with the Windows driver package).
- **`invalid configuration: no … preset for bitrate`** — the backend has no
  preset for that rate; pass explicit timing with `BusConfig::timing` /
  `BusConfig::fd_timing`.

## License

MIT
