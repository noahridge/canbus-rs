# canbus-core

The stable contract of the [`canbus`](https://crates.io/crates/canbus) crate
family: `CanMessage`, `BusConfig`, the `CanBus` trait, the `Interface`
backend descriptor and the vendor-neutral `Error`.

Applications should depend on `canbus`. Depend on `canbus-core` directly to
**write a backend**: implement `CanBus` and export
`pub const INTERFACE: Interface`, then users open it with
`canbus::Bus::open_with(&your_crate::INTERFACE, &config)`.

License: MIT
