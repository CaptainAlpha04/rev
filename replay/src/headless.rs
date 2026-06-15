use rev_core::error::RevError;
use std::path::Path;

pub struct HeadlessProcess {
    pub pid: u32,
    #[cfg(target_os = "linux")]
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

#[cfg(not(target_os = "linux"))]
pub fn spawn_headless(_program: &Path, _args: &[String]) -> Result<HeadlessProcess, RevError> {
    Err(RevError::UnsupportedPlatform(
        "Headless spawner is only supported on Linux".to_string(),
    ))
}
