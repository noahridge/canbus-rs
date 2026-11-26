# canbus-pcan

PEAK PCAN-Basic backend for the [`canbus`](https://crates.io/crates/canbus)
crate family (interface name `"pcan"`).

PCAN-Basic (`PCANBasic.dll` / `libpcanbasic.so`) is loaded at runtime;
install [PCAN-Basic](https://www.peak-system.com/PCAN-Basic.239.0.html).
Most applications should use this through `canbus` rather than directly.

> **Not yet tested on PEAK hardware.** Please report results, passing or
> failing, in [#26](https://github.com/noahridge/canbus-rs/issues/26).

License: MIT
