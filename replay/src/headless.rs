use rev_core::error::RevError;
use std::path::Path;

pub struct HeadlessProcess {
    pub pid: u32,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    pub child: std::process::Child,
}

#[cfg(target_os = "linux")]
pub fn spawn_headless(program: &Path, args: &[String]) -> Result<HeadlessProcess, RevError> {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new(program);
    cmd.args(args);

    unsafe {
        cmd.pre_exec(|| {
            libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            );
            libc::raise(libc::SIGSTOP);
            Ok(())
        });
    }

    let child = cmd.spawn()?;
    let pid = child.id();

    // Wait for the initial SIGSTOP signal
    let mut status = 0;
    unsafe {
        libc::waitpid(pid as libc::pid_t, &mut status, 0);
        libc::ptrace(
            libc::PTRACE_SETOPTIONS,
            pid as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            (libc::PTRACE_O_TRACESYSGOOD | libc::PTRACE_O_TRACEEXIT) as *mut libc::c_void,
        );
    }

    Ok(HeadlessProcess { pid, child })
}

#[cfg(target_os = "windows")]
pub fn spawn_headless(program: &Path, args: &[String]) -> Result<HeadlessProcess, RevError> {
    use std::os::windows::process::CommandExt;
    const CREATE_SUSPENDED: u32 = 0x00000004;

    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    cmd.creation_flags(CREATE_SUSPENDED);

    let child = cmd.spawn()?;
    let pid = child.id();

    Ok(HeadlessProcess { pid, child })
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn spawn_headless(_program: &Path, _args: &[String]) -> Result<HeadlessProcess, RevError> {
    Err(RevError::UnsupportedPlatform(
        "Headless spawner is only supported on Linux and Windows".to_string(),
    ))
}
