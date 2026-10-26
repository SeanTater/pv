//! Cooperative Unix termination: handlers only set an atomic; ordinary Rust
//! unwinding closes descriptors, anonymous staging files and IPC sockets.
use std::io;

#[cfg(unix)]
mod unix {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    static RECEIVED: AtomicI32 = AtomicI32::new(0);

    extern "C" fn handle(signal: libc::c_int) {
        // AtomicI32 is lock-free on the supported Unix targets. No allocation,
        // locking, stdio, or cleanup runs inside the signal handler.
        if RECEIVED
            .compare_exchange(0, signal, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            // A repeated request remains an escape from a stuck system call.
            unsafe { libc::_exit(128 + signal) };
        }
    }

    pub struct Handlers(Vec<(libc::c_int, libc::sigaction)>);
    impl Drop for Handlers {
        fn drop(&mut self) {
            for (signal, action) in self.0.iter().rev() {
                // SAFETY: action is the previous valid disposition for signal.
                unsafe {
                    libc::sigaction(*signal, action, std::ptr::null_mut());
                }
            }
        }
    }

    pub fn install() -> io::Result<Handlers> {
        let mut handlers = Handlers(Vec::new());
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            let mut previous = unsafe { std::mem::zeroed::<libc::sigaction>() };
            // SAFETY: a null action queries the current disposition.
            if unsafe { libc::sigaction(signal, std::ptr::null(), &mut previous) } != 0 {
                return Err(io::Error::last_os_error());
            }
            // Respect a parent's intentionally ignored signals (e.g. background jobs).
            if previous.sa_sigaction == libc::SIG_IGN {
                continue;
            }
            let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
            action.sa_sigaction = handle as *const () as usize;
            // No SA_RESTART: a blocked syscall must yield to cancellation.
            unsafe {
                libc::sigemptyset(&mut action.sa_mask);
            }
            if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            handlers.0.push((signal, previous));
        }
        Ok(handlers)
    }

    pub fn received() -> Option<i32> {
        let signal = RECEIVED.load(Ordering::Relaxed);
        (signal != 0).then_some(signal)
    }
}

#[cfg(unix)]
pub use unix::{install, received};
#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc, OnceLock,
    };
    use std::thread::JoinHandle;
    use std::time::Duration;
    type Handle = *mut c_void;

    // Kernel32 ABI: https://learn.microsoft.com/windows/console/setconsolectrlhandler
    // and https://learn.microsoft.com/windows/win32/fileio/cancelsynchronousio-func
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
        fn CreateEventW(
            attributes: *const c_void,
            manual: i32,
            initial: i32,
            name: *const u16,
        ) -> Handle;
        fn SetEvent(event: Handle) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn GetCurrentProcess() -> Handle;
        fn GetCurrentThread() -> Handle;
        fn DuplicateHandle(
            source_process: Handle,
            source: Handle,
            target_process: Handle,
            target: *mut Handle,
            access: u32,
            inherit: i32,
            options: u32,
        ) -> i32;
        fn CancelSynchronousIo(thread: Handle) -> i32;
        fn TerminateProcess(process: Handle, exit_code: u32) -> i32;
    }
    static RECEIVED: AtomicI32 = AtomicI32::new(0);
    // The event is process-lived so a concurrently running callback can never
    // use a closed/recycled handle during handler removal. One fixed handle,
    // not one allocation per event or per transferred buffer.
    static EVENT: OnceLock<OwnedHandle> = OnceLock::new();

    unsafe extern "system" fn handle(event: u32) -> i32 {
        if event > 1 {
            return 0;
        } // Close/logoff/shutdown retain OS semantics.
        if RECEIVED
            .compare_exchange(0, 2, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            unsafe {
                TerminateProcess(GetCurrentProcess(), 130);
            }
        }
        if let Some(event) = EVENT.get() {
            unsafe {
                SetEvent(event.as_raw_handle());
            }
        }
        1
    }
    pub struct Handlers {
        registered: bool,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }
    impl Drop for Handlers {
        fn drop(&mut self) {
            if self.registered {
                unsafe {
                    SetConsoleCtrlHandler(Some(handle), 0);
                }
            }
            self.stop.store(true, Ordering::Relaxed);
            if let Some(event) = EVENT.get() {
                unsafe {
                    SetEvent(event.as_raw_handle());
                }
            }
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
    pub fn install() -> io::Result<Handlers> {
        if EVENT.get().is_none() {
            let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            let event = unsafe { OwnedHandle::from_raw_handle(event) };
            let _ = EVENT.set(event);
        }
        let mut thread = std::ptr::null_mut();
        // Duplicate the main thread's pseudo-handle into a real, owned handle
        // usable by the cancellation worker (DUPLICATE_SAME_ACCESS = 2).
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                GetCurrentThread(),
                GetCurrentProcess(),
                &mut thread,
                0,
                0,
                2,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
        let mut handlers = Handlers {
            registered: false,
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        };
        if unsafe { SetConsoleCtrlHandler(Some(handle), 1) } == 0 {
            let error = io::Error::last_os_error();
            // No attached console is normal for services/redirected tools.
            if error.raw_os_error() == Some(6) {
                return Ok(handlers);
            }
            return Err(error);
        }
        handlers.registered = true;
        let stop = handlers.stop.clone();
        handlers.worker = Some(std::thread::Builder::new().name("pv-cancel".into()).spawn(
            move || {
                let event = EVENT.get().expect("event initialized before handler");
                unsafe {
                    WaitForSingleObject(event.as_raw_handle(), u32::MAX);
                }
                // Repeat until unwinding finishes: cancellation can race the
                // check immediately before ReadFile/WriteFile begins blocking.
                while !stop.load(Ordering::Relaxed) {
                    unsafe {
                        CancelSynchronousIo(thread.as_raw_handle());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            },
        )?);
        Ok(handlers)
    }
    pub fn received() -> Option<i32> {
        (RECEIVED.load(Ordering::Relaxed) != 0).then_some(2)
    }
}
#[cfg(windows)]
pub use windows::{install, received};
#[cfg(not(any(unix, windows)))]
pub fn install() -> io::Result<()> {
    Ok(())
}
#[cfg(not(any(unix, windows)))]
pub fn received() -> Option<i32> {
    None
}

pub fn check() -> io::Result<()> {
    if let Some(signal) = received() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            crate::error::Failure::new(
                128 + signal,
                format!("transfer cancelled by signal {signal}"),
            ),
        ))
    } else {
        Ok(())
    }
}
