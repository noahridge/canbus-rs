# Architecture

`canbus` follows the same shape as python-can: a vendor-neutral interface,
backends that implement it, and a facade that selects a backend at runtime.

## Crates

```
crates/
├── canbus-core/     The contract. No FFI, no OS code; changes rarely.
├── canbus/          Facade applications depend on. Re-exports canbus-core.
├── canbus-kvaser/   Kvaser CANlib backend
├── canbus-pcan/     PEAK PCAN-Basic backend
└── canbus-virtual/  In-process virtual bus
```

```
            application
                 │
              canbus  ──────────────┬──────────────┬───────────────┐
                 │                  │              │               │
            canbus-core ◄── canbus-kvaser   canbus-pcan   canbus-virtual
                 ▲
                 └── third-party backends (opened with Bus::open_with)
```

### Why several crates

- **A stable contract for third-party backends.** Rust treats the same trait
  from two semver-incompatible versions of a crate as different traits. Keeping
  `CanBus` in `canbus-core`, which only changes when the contract does, lets
  backends written against it keep working while `canbus` evolves.
- **In-tree backends prove the contract.** `canbus-kvaser` and `canbus-pcan`
  can only use `canbus-core`'s public API, so anything a third party would
  need is guaranteed to be public.
- **Dependency isolation.** Each backend carries only its own dependencies;
  applications trim backends with Cargo features on `canbus`.

### Why no `-sys` crates

`-sys` crates exist for libraries linked at build time. Both vendor libraries
are loaded at runtime with `libloading`, so each backend keeps its bindings in
a private `ffi` module. That keeps raw vendor types out of every public API
and lets bindings change without a semver break.

## canbus-core

| Module | Contents |
|---|---|
| `frame` | `CanMessage` (`Data` / `Remote` / `Error`), `MessageFlags`, DLC ⇄ length helpers |
| `config` | `BusConfig`, `Timing`, `FdConfig`; backend options as string key/values |
| `bus` | `CanBus` trait, `BusStatus`, `BusState`, `Capabilities`, `Deadline` |
| `filter` | `Filter` (code/mask or range), `FilterSupport` |
| `interface` | `Interface` descriptor, `ChannelInfo` |
| `error` | `Error`, `Result` |
| `embedded` | `embedded-can` impls and the `EmbeddedCan` adapter |

Design rules:

- **Vendor-neutral types only.** Vendor specifics live in
  `BusConfig::options` and in each backend's docs, never as fields or
  variants in the core.
- **Configure at open.** Bitrate, FD and listen-only are part of
  `BusConfig`, so backends whose drivers need the bitrate up front (PCAN)
  and those that set it after opening (Kvaser) look the same.
- **`Send`, not `Sync`.** Methods that touch the device take `&mut self`;
  a bus is never entered concurrently, matching Kvaser's threading rules.
- **One meaning per field.** `len()` is bytes and `dlc()` the DLC code;
  timestamps are `Duration`s. Backends convert at the FFI boundary.
- **Growth without breakage.** `Error`, `BusConfig`, `Capabilities`,
  `BusState` and `Interface` are `#[non_exhaustive]` and built through
  constructors.

## Backends

Each backend crate has the same layout:

| File | Role |
|---|---|
| `lib.rs` | Docs (channel format, options), `INTERFACE`, re-exports |
| `bus.rs` | The `CanBus` implementation and `detect()` |
| `convert.rs` | Flags, status codes, bitrate presets, timeouts |
| `ffi.rs` | Private bindings + runtime loader (`OnceLock`ed per process) |

Plus backend-specific helpers (`canbus-pcan`: `channel_name.rs` for
`PCAN_USBBUS1`-style names, `event.rs` for Win32-event / Linux-fd receive
notification with a polling fallback).

### Safety strategy

- All `unsafe` is confined to the backends' FFI calls, each with a `SAFETY`
  comment. Symbol types are inferred from the binding struct's fields, so
  each signature is declared exactly once.
- Function pointers are `extern "system"`: `__stdcall` on 32-bit Windows
  (as both vendor headers declare), the C ABI everywhere else.
- Handles are owned by the bus type and released in `Drop`.
- Library load failures are cached and reported as
  `Error::LibraryNotLoaded` — never a panic — and the crates build on every
  platform, including those the vendor doesn't support.

## canbus (facade)

- `interfaces()` is a `const` list of the compiled-in backends' descriptors;
  `Bus::open` looks one up by name. No global mutable registry.
- `Bus` wraps `Box<dyn CanBus>` and adds software filtering when a backend
  reports `FilterSupport::Software`, so filters behave identically on every
  backend.
- `config_from_env` / `config_from_lookup` implement python-can's `CAN_*`
  environment variables.

## Testing

- Unit tests in every crate run on any host (including macOS), since nothing
  needs a vendor library to compile.
- `crates/canbus/tests/virtual_bus.rs` exercises the facade end-to-end on the
  virtual backend.
- `crates/canbus/tests/hardware.rs` holds `#[ignore]`d smoke tests for Kvaser
  virtual channels and PCAN hardware, including moving a bus across threads.
