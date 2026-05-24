//! Platform-specific receive-event support for `read_wait`.
//!
//! PCAN-Basic offers no native blocking-with-timeout read. The portable
//! pattern is to obtain a per-channel notification handle and wait on it
//! with the platform primitive — but the two platforms differ in how the
//! handle is obtained:
//!
//! - **Linux:** `CAN_GetValue(channel, PCAN_RECEIVE_EVENT, ..)` returns a
//!   file descriptor the driver writes to; we `poll()` it. The fd is owned
//!   by the driver, so there is nothing to free.
//! - **Windows:** the application must *create* a Win32 event and register
//!   it via `CAN_SetValue(channel, PCAN_RECEIVE_EVENT, ..)`; the driver then
//!   signals it. `CAN_GetValue` does **not** hand back a waitable handle
//!   here, so we create one and `WaitForSingleObject` on it. The handle is
//!   ours, so [`release`] deregisters and closes it.
//!
//! [`acquire`] returns `None` when no usable handle is available, in which
//! case the caller falls back to polling.

use core::time::Duration;

use pcan_sys as sys;

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

/// A platform-specific receive-event handle.
///
/// On Windows this is a Win32 HANDLE we own; on Linux a file descriptor
/// owned by the driver.
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
        fn CreateEventA(
            attributes: *mut std::os::raw::c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const std::os::raw::c_char,
        ) -> EventHandle;
        fn CloseHandle(handle: EventHandle) -> i32;
    }

    pub(super) fn acquire(l: &sys::PcanLib, handle: sys::TPCANHandle) -> Option<EventHandle> {
        // Auto-reset, initially non-signalled. Auto-reset is correct here:
        // read_wait drains the queue with a plain read() before waiting, so
        // a single signal per arrival cannot lose a queued frame.
        let event =
            unsafe { CreateEventA(core::ptr::null_mut(), 0, 0, core::ptr::null()) };
        if event.is_null() {
            return None;
        }
        let mut h = event;
        // SAFETY: registering an owned, valid event handle with the driver.
        let status = unsafe {
            (l.CAN_SetValue)(
                handle,
                sys::PCAN_RECEIVE_EVENT,
                &mut h as *mut EventHandle as *mut std::os::raw::c_void,
                core::mem::size_of::<EventHandle>() as u32,
            )
        };
        if status == sys::PCAN_ERROR_OK {
            Some(event)
        } else {
            unsafe { CloseHandle(event) };
            None
        }
    }

    pub(super) fn release(l: &sys::PcanLib, handle: sys::TPCANHandle, event: EventHandle) {
        // Deregister (set a null handle) before closing ours.
        let mut zero: EventHandle = core::ptr::null_mut();
        // SAFETY: `event` was returned by `acquire` and not yet closed.
        unsafe {
            (l.CAN_SetValue)(
                handle,
                sys::PCAN_RECEIVE_EVENT,
                &mut zero as *mut EventHandle as *mut std::os::raw::c_void,
                core::mem::size_of::<EventHandle>() as u32,
            );
            CloseHandle(event);
        }
    }

    /// Win32's `INFINITE` sentinel — passing this as the timeout makes
    /// `WaitForSingleObject` block forever, which is not what callers asking
    /// for a finite (if very large) `Duration` want.
    const INFINITE: u32 = u32::MAX;

    pub(super) fn wait(handle: EventHandle, timeout: Duration) -> WaitOutcome {
        // Clamp below INFINITE so a saturating Duration doesn't accidentally
        // turn this into an infinite wait. The Linux poll() path uses
        // i32::MAX ms as its cap, so this keeps both platforms finite.
        let ms = timeout.as_millis().min((INFINITE - 1) as u128) as u32;
        // SAFETY: `handle` is an event we created and registered; it stays
        // valid until `release` is called.
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

    pub(super) fn acquire(l: &sys::PcanLib, handle: sys::TPCANHandle) -> Option<EventHandle> {
        let mut fd: EventHandle = 0;
        // SAFETY: writing a c_int-sized value the driver fills in.
        let status = unsafe {
            (l.CAN_GetValue)(
                handle,
                sys::PCAN_RECEIVE_EVENT,
                &mut fd as *mut EventHandle as *mut std::os::raw::c_void,
                core::mem::size_of::<EventHandle>() as u32,
            )
        };
        if status == sys::PCAN_ERROR_OK && fd != 0 {
            Some(fd)
        } else {
            None
        }
    }

    pub(super) fn release(_l: &sys::PcanLib, _handle: sys::TPCANHandle, _event: EventHandle) {
        // The fd belongs to the driver and is torn down by CAN_Uninitialize;
        // nothing to free here.
    }

    pub(super) fn wait(fd: EventHandle, timeout: Duration) -> WaitOutcome {
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let mut pfd = PollFd { fd, events: POLLIN, revents: 0 };
        // SAFETY: pfd is a valid stack object; poll only touches the buffer
        // we provide.
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

/// Acquire a receive-event handle for `handle`, or `None` to fall back to
/// polling. See the module docs for the per-platform mechanism.
pub(crate) fn acquire(l: &sys::PcanLib, handle: sys::TPCANHandle) -> Option<EventHandle> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::acquire(l, handle)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::acquire(l, handle)
    }
}

/// Release a handle previously returned by [`acquire`].
pub(crate) fn release(l: &sys::PcanLib, handle: sys::TPCANHandle, event: EventHandle) {
    #[cfg(target_os = "windows")]
    {
        windows_impl::release(l, handle, event)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::release(l, handle, event)
    }
}

/// Wait for the receive-event to become signalled, or for the timeout to
/// expire.
pub(crate) fn wait_for_event(handle: EventHandle, timeout: Duration) -> WaitOutcome {
    #[cfg(target_os = "windows")]
    {
        windows_impl::wait(handle, timeout)
    }
    #[cfg(target_os = "linux")]
    {
        linux_impl::wait(handle, timeout)
    }
}
