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
#[cfg(not(unix))]
pub fn install() -> io::Result<()> {
    Ok(())
}
#[cfg(not(unix))]
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
