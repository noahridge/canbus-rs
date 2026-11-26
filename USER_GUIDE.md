# canbus User Guide

## Contents

1. [Opening a bus](#opening-a-bus)
2. [Configuration](#configuration)
3. [Frames](#frames)
4. [Sending and receiving](#sending-and-receiving)
5. [Filters](#filters)
6. [Status and capabilities](#status-and-capabilities)
7. [Errors](#errors)
8. [Threads](#threads)
9. [Backend reference](#backend-reference)
10. [embedded-can](#embedded-can)
11. [Writing a backend](#writing-a-backend)
12. [Migrating from the `canlib` / `pcan` crates](#migrating-from-the-canlib--pcan-crates)

## Opening a bus

Every bus is described by a `BusConfig`: an **interface** (backend) name, a
**channel** in that backend's format, and settings.

```rust
use canbus::{Bus, BusConfig};

let bus = Bus::open(&BusConfig::new("kvaser", "0").bitrate(500_000))?;
let bus = Bus::open(&BusConfig::new("pcan", "PCAN_USBBUS1").bitrate(500_000))?;
let bus = Bus::open(&BusConfig::new("virtual", "vcan0"))?;
# Ok::<(), canbus::Error>(())
```

Other ways to open:

- `Bus::from_env()` — reads `CAN_INTERFACE`, `CAN_CHANNEL`, `CAN_BITRATE`,
  `CAN_DATA_BITRATE`, `CAN_FD`, `CAN_LISTEN_ONLY` (the python-can names).
  `canbus::config_from_lookup` does the same from any key/value source.
- `Bus::open_with(&some_crate::INTERFACE, &config)` — a backend that isn't
  compiled into `canbus`.
- `Bus::from_backend("name", backend_bus)` — wrap a backend bus you opened
  directly.

Discover what's available:

```rust
for interface in canbus::interfaces() {
    println!("{}: {}", interface.name, interface.description);
}
for ch in canbus::detect_available() {
    let bus = canbus::Bus::open(&ch.config().bitrate(500_000))?;
    # let _ = bus;
}
# Ok::<(), canbus::Error>(())
```

A bus is on-bus as soon as it's open. It goes off-bus on `shutdown()` or
when dropped.

## Configuration

```rust
use canbus::{BusConfig, Timing};

let cfg = BusConfig::new("pcan", "PCAN_USBBUS1")
    .bitrate(500_000)          // nominal bitrate, bit/s
    .fd(2_000_000)             // enable CAN FD with this data bitrate
    .listen_only(false)        // silent mode: no TX, no ACKs
    .option("key", "value");   // backend-specific options (see below)

// Explicit timing instead of a preset (tseg1 includes the propagation segment):
let cfg = BusConfig::new("kvaser", "0").timing(Timing {
    clock_hz: 80_000_000, brp: 10, tseg1: 12, tseg2: 3, sjw: 1,
});
```

Standard bitrates map to each vendor's presets. Rates without a preset either
let the driver derive timing (Kvaser) or return `Error::InvalidConfig`
explaining what to set instead.

## Frames

`CanMessage` is an enum of `Data`, `Remote` and `Error` frames.

```rust
use canbus::{Brs, CanMessage, ExtendedId, StandardId};

let classic = CanMessage::new(StandardId::new(0x123).unwrap(), &[1, 2, 3])?;
let fd = CanMessage::new_fd(ExtendedId::new(0x18DA_F110).unwrap(), &[0; 20], Brs::On)?;
let rtr = CanMessage::new_remote(StandardId::new(0x7DF).unwrap(), 8)?;

assert_eq!(fd.len(), 24);   // payload bytes (padded up to a valid FD length)
assert_eq!(fd.dlc(), 12);   // the 4-bit DLC code
# Ok::<(), canbus::Error>(())
```

- `id()` is the raw identifier; `typed_id()` returns an `embedded_can::Id`.
- `len()` is always bytes and `dlc()` always the DLC code, on every backend.
- `timestamp()` is set on received frames as a `Duration` from a
  backend-defined epoch; compare timestamps from the same bus.
- `is_echo()` marks a frame this bus transmitted, when echoes are enabled.

## Sending and receiving

Timeouts follow one rule everywhere: `None` waits forever,
`Some(Duration::ZERO)` never blocks, `Some(d)` waits up to `d`.

```rust
use std::time::Duration;
use canbus::CanBus;
# fn demo(bus: &mut canbus::Bus, msg: &canbus::CanMessage) -> canbus::Result<()> {
bus.send(msg, Some(Duration::from_millis(100)))?;   // waits for TX-queue space

match bus.recv(Some(Duration::from_secs(1)))? {
    Some(frame) if frame.is_error_frame() => eprintln!("bus error: {:?}", frame.flags()),
    Some(frame) => println!("{:X}: {:02X?}", frame.id(), frame.data()),
    None => println!("timeout"),
}
# Ok(()) }
```

`send` returns `Error::TxQueueFull` (zero timeout) or `Error::Timeout` when
the transmit queue stays full. `recv` returns error frames as frames, not as
`Err`.

## Filters

```rust
use canbus::{CanBus, Filter};
# fn demo(bus: &mut canbus::Bus) -> canbus::Result<()> {
bus.set_filters(&[
    Filter::standard(0x7E8),
    Filter::Range { from: 0x100, to: 0x1FF, extended: false },
    Filter::CodeMask { code: 0x18DA_F100, mask: 0x1FFF_FF00, extended: true },
])?;
bus.set_filters(&[])?; // accept everything again
# Ok(()) }
```

A frame passes if **any** filter matches; error frames always pass. Backends
apply what their hardware can and report `FilterSupport::Software` when it
isn't exact; `Bus` then filters the rest in software, so results are
identical on every backend.

## Status and capabilities

```rust
use canbus::{BusState, CanBus};
# fn demo(bus: &canbus::Bus) -> canbus::Result<()> {
let status = bus.status()?;
if status.state == BusState::BusOff { /* … */ }
if let Some(c) = status.error_counters { println!("TEC {} REC {}", c.tx, c.rx); }

let caps = bus.capabilities();
println!("FD: {}, hardware filters: {}", caps.fd, caps.hardware_filters);
# Ok(()) }
```

## Errors

All calls return `canbus::Result<T>` with a vendor-neutral `canbus::Error`.
Common variants: `LibraryNotLoaded`, `UnknownInterface`, `ChannelNotFound`,
`ChannelInUse`, `InvalidConfig`, `InvalidFrame`, `Unsupported`, `Timeout`,
`TxQueueFull`, `BusOff`, `Shutdown`. Anything else is
`Error::Backend { interface, code, message }` with the vendor's own code and
message text.

## Threads

Every bus is `Send` but not `Sync`: you can move a bus to another thread,
but not use one bus from two threads at once. This matches vendor rules
(Kvaser forbids concurrent calls on one handle). To send and receive from
different threads, open two buses on the same channel — on hardware, each
bus has its own handle.

```rust
let rx_bus = canbus::Bus::open(&canbus::BusConfig::new("virtual", "vcan0"))?;
let reader = std::thread::spawn(move || {
    let mut bus = rx_bus;
    // … bus.recv(…) …
    # let _ = &mut bus;
});
# reader.join().unwrap();
# Ok::<(), canbus::Error>(())
```

## Backend reference

### `kvaser`

- **Channel:** CANlib channel index (`"0"`, `"1"`, …).
- **Bitrates:** presets for 1M, 500k, 250k, 125k, 100k, 83.3k, 62.5k, 50k,
  10k; other rates are derived by CANlib. FD data presets: 500k, 1M, 2M, 4M,
  8M (80 % sample point).
- **Options:** `accept_virtual` (default `true`), `exclusive` (`false`),
  `fd_non_iso` (`false`).
- **Timestamps:** 10 µs resolution where supported, else 1 ms.
- **Library:** `canlib32.dll` / `libcanlib.so`; override with `CANLIB_SDK_DIR`
  (`bin_x64` / `Bin` on Windows, `lib` on Linux).

### `pcan`

> **Not yet tested on PEAK hardware.** The pure logic is unit-tested, but the
> calls into PCAN-Basic haven't been run against a real adapter. Please report
> results, passing or failing, in
> [#26](https://github.com/noahridge/canbus-rs/issues/26).

- **Channel:** `PCAN_USBBUS1`–`16`, `PCAN_PCIBUS1`–`16`, `PCAN_LANBUS1`–`16`,
  or a raw handle (`0x51`).
- **Bitrate is required.** Classic presets: 1M, 800k, 500k, 250k, 125k, 100k,
  95k, 83k, 50k, 47k, 33k, 20k, 10k, 5k; explicit classic timing must use the
  8 MHz SJA1000 clock. FD presets (80 MHz clock): nominal 125k/250k/500k/1M,
  data 500k/1M/2M/4M/8M.
- **Timestamps:** 1 µs resolution.
- **Not supported:** `flush_tx` (PCAN's only flush also drops received frames).
- **Library:** `PCANBasic.dll` / `libpcanbasic.so`; override with
  `PCAN_SDK_DIR` (`x64` / `x86` on Windows).

### `virtual`

- **Channel:** any string. Every bus on the same name in the process is
  connected.
- Classic CAN unless the config enables FD; sending FD frames on a classic
  bus fails as it would on hardware.
- **Options:** `receive_own_messages` (default `false`).

## embedded-can

`Bus` implements `embedded_can::blocking::Can` and `embedded_can::nb::Can`.
For any other `CanBus`, wrap it: `canbus::EmbeddedCan(my_bus)`. Error frames
surface as `EmbeddedCanError::Bus` with the decoded `embedded_can::ErrorKind`.

## Writing a backend

Depend on `canbus-core`, implement `CanBus` for your bus type, and export a
descriptor:

```rust
use canbus_core::{BusConfig, CanBus, ChannelInfo, Interface, Result};

fn open(config: &BusConfig) -> Result<Box<dyn CanBus>> {
    # let _ = config; unimplemented!()
    // Ok(Box::new(MyBus::open(config)?))
}

fn detect() -> Result<Vec<ChannelInfo>> {
    Ok(Vec::new())
}

pub const INTERFACE: Interface = Interface::new("mybackend", "My CAN adapter", open, detect);
```

Users open it with `canbus::Bus::open_with(&mybackend::INTERFACE, &config)`.
Follow the trait's timeout, filter and threading rules; `canbus_core::Deadline`
helps with polling loops. The `canbus-virtual` crate is a compact reference
implementation.

## Migrating from the `canlib` / `pcan` crates

| Before | After |
|---|---|
| `canlib::Channel::open(0, OpenFlags::ACCEPT_VIRTUAL)` + `set_bitrate` + `bus_on` | `Bus::open(&BusConfig::new("kvaser", "0").bitrate(500_000))` |
| `pcan::PcanChannel::open(PcanBus::Usb(1))` + `set_bitrate` + `bus_on` | `Bus::open(&BusConfig::new("pcan", "PCAN_USBBUS1").bitrate(500_000))` |
| `Bitrate::Rate500K` / `FdBitrate::Rate2M80P` | `.bitrate(500_000)` / `.fd(2_000_000)` |
| `ch.write(&msg)` / `ch.write_wait(&msg, t)` | `bus.send(&msg, Some(Duration::ZERO))` / `bus.send(&msg, Some(t))` |
| `ch.read()` → `Err(CanError::NoMsg)` | `bus.recv(Some(Duration::ZERO))` → `Ok(None)` |
| `ch.read_wait(t)` → `Err(CanError::Timeout)` | `bus.recv(Some(t))` → `Ok(None)` |
| `ch.set_acceptance_filter(&f)` | `bus.set_filters(&[f])` |
| `ch.read_status()` / `read_error_counters()` | `bus.status()` |
| `set_output_control(DriverType::Silent)` | `.listen_only(true)` |
| `msg.dlc()` (bytes) | `msg.len()` (bytes); `msg.dlc()` is now the DLC code |
| `msg.timestamp()` → `Option<u64>` | `Option<Duration>` |
| `CanError` | `Error` |
| `can_core::mock::MockChannel` | the `virtual` interface |

Removed with no direct replacement: `read_specific`, `write_sync`, bus
statistics, raw handles and vendor status-code helpers. These were
Kvaser-only; open an issue if you need one back in a vendor-neutral form.
