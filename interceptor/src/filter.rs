use rev_core::types::SyscallKind;

#[derive(Default)]
pub struct SyscallFilter;

impl SyscallFilter {
    pub fn new() -> Self {
        Self
    }

    /// Determines if a captured SyscallKind should be recorded or ignored.
    pub fn should_capture(&self, kind: &SyscallKind) -> bool {
        match kind {
            SyscallKind::TimeRead => true,
            SyscallKind::RandomRead => true,
            SyscallKind::NetworkRead { .. } => true,
            SyscallKind::FileRead { path } => {
                if let Some(p) = path {
                    // Ignore noisy virtual filesystem reads that do not affect program execution state
                    if p.starts_with("/proc/") || p.starts_with("/sys/") || p == "/dev/null" {
                        return false;
                    }
                }
                true
            }
            SyscallKind::EnvRead { .. } => true,
            SyscallKind::ProcessId => true,
        }
    }
}
