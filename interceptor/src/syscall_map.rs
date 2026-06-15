use rev_core::types::SyscallKind;

/// Map raw Linux x86_64 syscall numbers to SyscallKind categories.
/// Unmapped syscalls return None, indicating they are not recorded for time-travel.
pub fn map_syscall(sys_num: u64) -> Option<SyscallKind> {
    match sys_num {
        0 => Some(SyscallKind::FileRead { path: None }), // sys_read (could be file, network, etc.)
        45 => Some(SyscallKind::NetworkRead { socket_addr: None }), // sys_recvfrom
        47 => Some(SyscallKind::NetworkRead { socket_addr: None }), // sys_recvmsg
        96 => Some(SyscallKind::TimeRead),               // sys_gettimeofday
        228 => Some(SyscallKind::TimeRead),              // sys_clock_gettime
        318 => Some(SyscallKind::RandomRead),            // sys_getrandom
        39 => Some(SyscallKind::ProcessId),              // sys_getpid
        _ => None,
    }
}
