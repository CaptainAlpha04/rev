use crate::args::CliArgs;
use rev_core::error::RevError;
use rev_core::types::RecorderConfig;
use rev_delta::DeltaEngine;
use rev_interceptor::create_interceptor;
use rev_recorder::Recorder;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

#[cfg(target_os = "linux")]
static CHILD_CRASHED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
extern "C" fn handle_sigchld(_sig: libc::c_int) {
    // Only async-signal-safe code in signal handler
    CHILD_CRASHED.store(true, Ordering::SeqCst);
}

#[cfg(target_os = "linux")]
fn setup_signal_handler() {
    unsafe {
        libc::signal(libc::SIGCHLD, handle_sigchld as libc::sighandler_t);
    }
}

#[cfg(not(target_os = "linux"))]
fn setup_signal_handler() {}

pub fn run_orchestrator(args: CliArgs) -> Result<(), RevError> {
    // 1. Resolve trace output directory
    let trace_dir = if args.output.starts_with('~') {
        let mut path = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
            PathBuf::from(userprofile)
        } else {
            PathBuf::from(".")
        };
        let rem = args.output.trim_start_matches('~').trim_start_matches('/');
        if !rem.is_empty() {
            path.push(rem);
        }
        path
    } else {
        PathBuf::from(&args.output)
    };

    // If it's a replay command, handle replay immediately
    if let Some(trace_path) = args.replay {
        run_tui(&trace_path)?;
        return Ok(());
    }

    // If export command, handle export immediately
    if let Some(export_args) = args.export {
        if export_args.len() < 2 {
            return Err(RevError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Usage: --export <TRACE> <STEP>",
            )));
        }
        let trace_path = PathBuf::from(&export_args[0]);
        let step_id: u64 = export_args[1]
            .parse()
            .map_err(|e| RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

        let reader = rev_recorder::TraceReader::new(&trace_path)?;
        let introspector = get_introspector_by_name(&reader.header.runtime_name)?;

        let mut replay = rev_replay::ReplayEngine::new(&trace_path, introspector)?;
        let state = replay.state_at(step_id)?;

        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

        println!("{}", json);
        return Ok(());
    }

    // 2. Parse command to execute
    let command_str = args.runtime.ok_or(RevError::NoCommand)?;
    let mut parts = vec![command_str];
    parts.extend(args.passthrough_args);

    // Detect runtime
    let introspector = crate::runtime_detect::detect_runtime(&parts)?;
    let runtime_name = introspector.runtime_name().to_string();

    let program = &parts[0];
    let program_args = &parts[1..];

    setup_signal_handler();

    let start_ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    #[cfg(target_os = "linux")]
    let child_pid = {
        use std::os::unix::process::CommandExt;
        let mut cmd = std::process::Command::new(program);
        cmd.args(program_args);
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

        let mut status = 0;
        unsafe {
            libc::waitpid(pid as libc::pid_t, &mut status, 0);
        }
        pid
    };

    #[cfg(not(target_os = "linux"))]
    let child_pid = {
        // On Windows/Mock, we spawn the process normally.
        let mut cmd = std::process::Command::new(program);
        cmd.args(program_args);
        let child = cmd.spawn()?;
        child.id()
    };

    // Generate unique trace file name: <pid>_<timestamp>.rev-trace
    let trace_filename = format!("{}_{}.rev-trace", child_pid, start_ts);
    let trace_path = trace_dir.join(trace_filename);

    let config = RecorderConfig {
        trace_path: trace_path.clone(),
        chunk_size: rev_core::constants::DEFAULT_CHUNK_SIZE,
        runtime_name,
        target_pid: child_pid,
        start_ts,
    };

    let mut recorder = Recorder::new(&trace_path, &config)?;
    let mut delta = DeltaEngine::new(child_pid)?;
    let mut interceptor = create_interceptor();

    interceptor.attach(child_pid)?;

    if args.verbose {
        println!(
            "rev: Recording started for PID {} -> {:?}",
            child_pid, trace_path
        );
    }

    #[allow(clippy::while_let_loop)]
    loop {
        #[cfg(target_os = "linux")]
        {
            if CHILD_CRASHED.load(Ordering::SeqCst) {
                break;
            }
        }

        match interceptor.next_event() {
            Ok(event) => {
                recorder.record(event.clone())?;
                delta.commit_step(event.id)?;
            }
            Err(_) => {
                // Child exited or failed
                break;
            }
        }
    }

    let stats = recorder.finalize()?;

    if args.verbose {
        println!("rev: Recording finalized.");
        println!("  Total events:       {}", stats.total_events);
        println!("  Bytes written:      {}", stats.bytes_written);
        println!("  Compression ratio:  {:.2}x", stats.compression_ratio);
        println!("  Duration:           {}ms", stats.duration_ms);
    }

    #[cfg(target_os = "linux")]
    let crashed = rev_interceptor::CHILD_EXITED_ABNORMALLY.load(std::sync::atomic::Ordering::SeqCst);
    #[cfg(not(target_os = "linux"))]
    let crashed = false;

    if crashed && !args.no_tui {
        run_tui(&trace_path)?;
    }

    Ok(())
}

fn run_tui(trace_path: &std::path::Path) -> Result<(), RevError> {
    // 1. Resolve introspector from trace header
    let reader = rev_recorder::TraceReader::new(trace_path)?;
    let introspector = get_introspector_by_name(&reader.header.runtime_name)?;

    // 2. Initialize terminal guard (enabling raw mode, alternate screen)
    struct TerminalGuard;
    impl TerminalGuard {
        fn new() -> Result<Self, std::io::Error> {
            crossterm::terminal::enable_raw_mode()?;
            crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
            Ok(TerminalGuard)
        }
    }
    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }

    let _guard = TerminalGuard::new().map_err(RevError::Io)?;

    // 3. Construct terminal and run app
    let mut stdout = std::io::stdout();
    let backend = ratatui::backend::CrosstermBackend::new(&mut stdout);
    let mut terminal = ratatui::Terminal::new(backend).map_err(RevError::Io)?;

    let replay = rev_replay::ReplayEngine::new(trace_path, introspector)?;
    let mut app = rev_tui::TuiApp::new(replay, trace_path.to_path_buf());

    app.run(&mut terminal)?;
    Ok(())
}

fn get_introspector_by_name(
    name: &str,
) -> Result<Box<dyn rev_replay::RuntimeIntrospector>, RevError> {
    match name {
        "python3" | "python" => Ok(Box::new(rev_replay::PythonIntrospector::new())),
        "node" => Ok(Box::new(rev_replay::NodeIntrospector::new())),
        "ruby" => Ok(Box::new(rev_replay::RubyIntrospector::new())),
        other => Err(RevError::UnsupportedRuntime(other.to_string())),
    }
}
