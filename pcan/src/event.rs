//! Platform-specific event waiting for `read_wait`.
//!
//! PCAN-Basic does not offer a native blocking-with-timeout read. The
//! recommended pattern is to obtain a notification handle via
//! `CAN_GetValue(channel, PCAN_RECEIVE_EVENT, &handle, ...)` and then
//! wait on it with the platform's primitive.
//!
//! This module exposes a single [`wait_for_event`] that takes the
//! per-channel event handle and a timeout, returning [`WaitOutcome`].
//! Backends translate the outcome to the appropriate result.

use core::time::Duration;

/// Outcome of a single wait on the receive-event handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitOutcome {
    /// The event was signalled — a message is (likely) ready.
    Signalled,
    /// The timeout expired before the event was signalled.
    Timeout,
    /// The wait itself failed (e.g. invalid handle).
    Failed,
}

/// A platform-specific receive-event handle obtained from
/// `CAN_GetValue(.., PCAN_RECEIVE_EVENT, ..)`.
///
/// On Windows this is a Win32 HANDLE; on Linux a file descriptor.
#[cfg(target_os = "windows")]
pub(crate) type EventHandle = *mut std::os::raw::c_void;

#[cfg(target_os = "linux")]
pub(crate) type EventHandle = std::os::raw::c_int;

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;

    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 0x102;

    #[link(name = "kernel32")]
    extern "system" {
        fn WaitForSingleObject(handle: EventHandle, milliseconds: u32) -> u32;
    }

    pub(super) fn wait(handle: EventHandle, timeout: Duration) -> WaitOutcome {
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        // SAFETY: handle came from CAN_GetValue(PCAN_RECEIVE_EVENT) which
        // returns a valid Win32 event handle owned by the PCAN driver for
        // the lifetime of the initialised channel.
        let r = unsafe { WaitForSingleObject(handle, ms) };
        match r {
            WAIT_OBJECT_0 => WaitOutcome::Signalled,
            WAIT_TIMEOUT => WaitOutcome::Timeout,
            _ => WaitOutcome::Failed,
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;

    #[repr(C)]
    struct PollFd {
        fd: std::os::raw::c_int,
        events: std::os::raw::c_short,
        revents: std::os::raw::c_short,
    }

    const POLLIN: std::os::raw::c_short = 0x0001;

    extern "C" {
        fn poll(
            fds: *mut PollFd,
            nfds: std::os::raw::c_ulong,
            timeout_ms: std::os::raw::c_int,
        ) -> std::os::raw::c_int;
    }

    pub(super) fn wait(fd: EventHandle, timeout: Duration) -> WaitOutcome {
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let mut pfd = PollFd { fd, events: POLLIN, revents: 0 };
        // SAFETY: pfd is a valid stack object; poll is a kernel syscall
        // shim that won't touch memory beyond the buffer we provide.
        let r = unsafe { poll(&mut pfd as *mut _, 1, ms) };
        if r > 0 && pfd.revents & POLLIN != 0 {
            WaitOutcome::Signalled
        } else if r == 0 {
            WaitOutcome::Timeout
        } else {
            WaitOutcome::Failed
        }
    }
}

/// Wait for the receive-event to become signalled, or for the timeout to
/// expire.
pub(crate) fn wait_for_event(handle: EventHandle, timeout: Duration) -> WaitOutcome {
    #[cfg(target_os = "windows")]
    { windows_impl::wait(handle, timeout) }
    #[cfg(target_os = "linux")]
    { linux_impl::wait(handle, timeout) }
}
