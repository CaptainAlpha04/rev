use crate::filter::SyscallFilter;
use crate::syscall_map::map_syscall;
use crate::Interceptor;
use rev_core::error::RevError;
use rev_core::types::{SyscallEvent, SyscallKind};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::SystemTime;

pub struct LinuxInterceptor {
    pid: Option<u32>,
    filter: SyscallFilter,
    in_syscall: bool,
    next_event_id: u64,
}

impl Default for LinuxInterceptor {
    fn default() -> Self {
        Self {
            pid: None,
            filter: SyscallFilter::new(),
            in_syscall: false,
            next_event_id: 0,
        }
    }
}

impl LinuxInterceptor {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Interceptor for LinuxInterceptor {
    fn attach(&mut self, pid: u32) -> Result<(), RevError> {
        self.pid = Some(pid);
        unsafe {
            if libc::ptrace(
                libc::PTRACE_ATTACH,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            ) < 0
            {
                return Err(RevError::AttachFailed {
                    pid,
                    reason: std::io::Error::last_os_error().to_string(),
                });
            }

            let mut status = 0;
            if libc::waitpid(pid as libc::pid_t, &mut status, 0) < 0 {
                return Err(RevError::AttachFailed {
                    pid,
                    reason: std::io::Error::last_os_error().to_string(),
                });
            }

            libc::ptrace(
                libc::PTRACE_SETOPTIONS,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                (libc::PTRACE_O_TRACESYSGOOD | libc::PTRACE_O_TRACEEXIT) as *mut libc::c_void,
            );
        }
        Ok(())
    }

    fn next_event(&mut self) -> Result<SyscallEvent, RevError> {
        let pid = self.pid.ok_or(RevError::AttachFailed {
            pid: 0,
            reason: "Not attached to any process".to_string(),
        })?;

        unsafe {
            loop {
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
                    let abnormally = if libc::WIFSIGNALED(status) {
                        true
                    } else {
                        libc::WEXITSTATUS(status) != 0
                    };
                    if abnormally {
                        crate::CHILD_EXITED_ABNORMALLY.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    return Err(RevError::ReplayFailed {
                        step: self.next_event_id,
                        reason: "Target process exited".to_string(),
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

                    let sys_num = regs.orig_rax;
                    self.in_syscall = !self.in_syscall;

                    // Capture on exit
                    if !self.in_syscall {
                        if let Some(mut kind) = map_syscall(sys_num) {
                            if self.filter.should_capture(&kind) {
                                let retval = regs.rax;
                                let mut return_bytes = Vec::new();
                                let mut fd = None;

                                match &mut kind {
                                    SyscallKind::FileRead { path } => {
                                        let fd_arg = regs.rdi as i32;
                                        fd = Some(fd_arg);
                                        if let Ok(p) = get_fd_path(pid, fd_arg) {
                                            *path = Some(p);
                                        }
                                        if retval > 0 && retval < 1_000_000 {
                                            return_bytes =
                                                read_child_memory(pid, regs.rsi, retval as usize)?;
                                        }
                                    }
                                    SyscallKind::NetworkRead { .. } => {
                                        fd = Some(regs.rdi as i32);
                                        if retval > 0 && retval < 1_000_000 {
                                            return_bytes =
                                                read_child_memory(pid, regs.rsi, retval as usize)?;
                                        }
                                    }
                                    SyscallKind::RandomRead => {
                                        if retval > 0 && retval < 1_000_000 {
                                            return_bytes =
                                                read_child_memory(pid, regs.rdi, retval as usize)?;
                                        }
                                    }
                                    SyscallKind::TimeRead => {
                                        if retval == 0 {
                                            if sys_num == 96 {
                                                return_bytes =
                                                    read_child_memory(pid, regs.rdi, 16)?;
                                            } else if sys_num == 228 {
                                                return_bytes =
                                                    read_child_memory(pid, regs.rsi, 16)?;
                                            }
                                        }
                                    }
                                    SyscallKind::ProcessId => {
                                        return_bytes = (retval as u32).to_le_bytes().to_vec();
                                    }
                                    _ => {}
                                }

                                let event = SyscallEvent {
                                    id: self.next_event_id,
                                    timestamp_ns: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_nanos()
                                        as u64,
                                    syscall: kind,
                                    return_bytes,
                                    fd,
                                };
                                self.next_event_id += 1;
                                return Ok(event);
                            }
                        }
                    }
                }
            }
        }
    }

    fn detach(&mut self) -> Result<(), RevError> {
        if let Some(pid) = self.pid.take() {
            unsafe {
                if libc::ptrace(
                    libc::PTRACE_DETACH,
                    pid as libc::pid_t,
                    std::ptr::null_mut::<libc::c_void>(),
                    std::ptr::null_mut::<libc::c_void>(),
                ) < 0
                {
                    return Err(RevError::Io(std::io::Error::last_os_error()));
                }
            }
        }
        Ok(())
    }
}

fn read_child_memory(pid: u32, address: u64, size: usize) -> Result<Vec<u8>, RevError> {
    let mem_path = format!("/proc/{}/mem", pid);
    let mut file = File::open(&mem_path)?;
    file.seek(SeekFrom::Start(address))?;
    let mut buffer = vec![0; size];
    file.read_exact(&mut buffer)?;
    Ok(buffer)
}

fn get_fd_path(pid: u32, fd: i32) -> Result<String, std::io::Error> {
    let link_path = format!("/proc/{}/fd/{}", pid, fd);
    let path = fs::read_link(link_path)?;
    Ok(path.to_string_lossy().into_owned())
}
