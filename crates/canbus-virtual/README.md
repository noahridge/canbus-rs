# canbus-virtual

In-process virtual CAN bus for the [`canbus`](https://crates.io/crates/canbus)
crate family (interface name `"virtual"`). Every bus opened on the same
channel name in a process is connected — ideal for tests and CI on any OS.

License: MIT
