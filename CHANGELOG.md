# Changelog

All notable changes to the `canbus` crates are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
crates follow [Semantic Versioning](https://semver.org/). All crates in the
workspace share one version.

## [Unreleased]

### Changed

- Minimum supported Rust version raised from 1.85 to 1.89, in preparation
  for a SocketCAN backend built on the `socketcan` crate (4.0 requires 1.89;
  see [#25](https://github.com/noahridge/canbus-rs/issues/25)).

## [0.1.0] - 2026-11-26

First release of the `canbus` crate family.

### Added

- `canbus`: facade with runtime backend selection (`Bus::open`,
  `Bus::open_with`, `Bus::from_env` with python-can's `CAN_*` variables),
  `interfaces()`, `detect_available()`, software filtering fallback and
  `embedded-can` impls. Cargo features `kvaser`, `pcan`, `virtual`
  (all default).
- `canbus-core`: the backend contract — `CanBus` (`Send`, not `Sync`),
  `CanMessage`, `BusConfig`, `Timing`, `Filter`, `Interface`, `Error` and the
  `EmbeddedCan` adapter.
- `canbus-kvaser`: Kvaser CANlib backend (Windows, Linux), loaded at runtime.
- `canbus-pcan`: PEAK PCAN-Basic backend (Windows, Linux), loaded at runtime.
- `canbus-virtual`: in-process virtual bus for tests and CI on any OS.
- Rust 2024 edition; minimum supported Rust version 1.85.

### Changed (relative to the unpublished `canlib` / `pcan` crates)

- Replaced the vendor-specific `Channel` / `PcanChannel` APIs with
  `BusConfig` + `CanBus`; see the migration table in `USER_GUIDE.md`.
- `CanMessage::len()` is the payload length in bytes and `dlc()` the DLC
  code on every backend; timestamps are `Duration`s.

### Fixed

- Kvaser range filters that weren't power-of-two aligned were translated to
  a code/mask accepting IDs outside the range.
- Function pointers now use the `__stdcall` ABI on 32-bit Windows.
- PCAN CAN FD frames reported the DLC code as their length.
- Kvaser timestamps were milliseconds but documented as microseconds.
- PCAN "channel in use" was reported as "invalid handle".
- The crates no longer fail to compile on platforms without vendor
  libraries (e.g. macOS); opening a bus reports `LibraryNotLoaded` instead.

### Known limitations

- The `pcan` backend has not been tested on PEAK hardware yet; its pure
  logic is unit-tested. Tracked in
  [#26](https://github.com/noahridge/canbus-rs/issues/26). The `kvaser`
  backend was validated on Windows with Kvaser's virtual channels (classic
  CAN, CAN FD, and a bus used from another thread).

[Unreleased]: https://github.com/noahridge/canbus-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/noahridge/canbus-rs/releases/tag/v0.1.0
