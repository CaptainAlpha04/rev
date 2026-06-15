use crate::HeadlessProcess;
use rev_core::error::RevError;
use rev_core::types::SyscallEvent;

#[cfg(target_os = "linux")]
pub fn fast_forward_process(
    proc: &mut HeadlessProcess,
    events: &[SyscallEvent],
    target_step: u64,
) -> Result<(), RevError> {
    let pid = proc.pid;
    let mut step = 0;

    unsafe {
        while step <= target_step {
            // Restart child and stop at next syscall entry/exit
            if libc::ptrace(
                libc::PTRACE_SYSCALL,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            ) < 0
            {
                return Err(RevError::Io(std::io::Error::last_os_error()));
            }

            let mut status = 0;
            if libc::waitpid(pid as libc::pid_t, &mut status, 0) < 0 {
                return Err(RevError::Io(std::io::Error::last_os_error()));
            }

            if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
                return Err(RevError::ReplayFailed {
                    step,
                    reason: "Headless process terminated early during replay".to_string(),
                });
            }

            if libc::WIFSTOPPED(status) && (libc::WSTOPSIG(status) & 0x7f) == libc::SIGTRAP {
                let mut regs: libc::user_regs_struct = std::mem::zeroed();
                if libc::ptrace(
                    libc::PTRACE_GETREGS,
                    pid as libc::pid_t,
                    std::ptr::null_mut::<libc::c_void>(),
                    &mut regs as *mut _ as *mut libc::c_void,
                ) < 0
                {
                    return Err(RevError::Io(std::io::Error::last_os_error()));
                }

                // We are at syscall entry or exit. We inject on exit (when result is returned to child)
                // Let's toggle step/injection
                let event = events.iter().find(|e| e.id == step);
                if let Some(ev) = event {
                    // Inject return value (override RAX register) and write return bytes into memory
                    regs.rax = ev.return_bytes.len() as u64; // mock success size or specific code
                                                             // If read/recv, write return_bytes to the buffer address (rsi)
                    if !ev.return_bytes.is_empty() {
                        let addr = regs.rsi; // buffer address on x86_64
                        write_child_memory(pid, addr, &ev.return_bytes)?;
                    }

                    if libc::ptrace(
                        libc::PTRACE_SETREGS,
                        pid as libc::pid_t,
                        std::ptr::null_mut::<libc::c_void>(),
                        &regs as *const _ as *mut libc::c_void,
                    ) < 0
                    {
                        return Err(RevError::Io(std::io::Error::last_os_error()));
                    }
                }
                step += 1;
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn write_child_memory(pid: u32, address: u64, bytes: &[u8]) -> Result<(), RevError> {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    let mem_path = format!("/proc/{}/mem", pid);
    let mut file = OpenOptions::new().write(true).open(&mem_path)?;
    file.seek(SeekFrom::Start(address))?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn fast_forward_process(
    _proc: &mut HeadlessProcess,
    _events: &[SyscallEvent],
    _target_step: u64,
) -> Result<(), RevError> {
    Err(RevError::UnsupportedPlatform(
        "Replay process is only supported on Linux".to_string(),
    ))
}
