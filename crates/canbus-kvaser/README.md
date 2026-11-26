# canbus-kvaser

Kvaser CANlib backend for the [`canbus`](https://crates.io/crates/canbus)
crate family (interface name `"kvaser"`).

CANlib (`canlib32.dll` / `libcanlib.so`) is loaded at runtime; install the
[Kvaser drivers or CANlib SDK](https://kvaser.com/developer/canlib-sdk/). Most
applications should use this through `canbus` rather than directly.

License: MIT
