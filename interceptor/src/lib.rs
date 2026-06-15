pub mod filter;
pub mod platform;
pub mod syscall_map;

use rev_core::error::RevError;
use rev_core::types::SyscallEvent;
use std::sync::atomic::AtomicBool;

pub static CHILD_EXITED_ABNORMALLY: AtomicBool = AtomicBool::new(false);

pub trait Interceptor: Send {
    /// Attach to a running process by PID
    fn attach(&mut self, pid: u32) -> Result<(), RevError>;

    /// Block until the next capturable event occurs, then return it
    fn next_event(&mut self) -> Result<SyscallEvent, RevError>;

    /// Detach cleanly without killing the process
    fn detach(&mut self) -> Result<(), RevError>;
}

/// Create the platform-appropriate Interceptor implementation
pub fn create_interceptor() -> Box<dyn Interceptor> {
    #[cfg(target_os = "linux")]
    {
        Box::new(platform::linux::LinuxInterceptor::new())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(platform::mock::MockInterceptor::new())
    }
}
