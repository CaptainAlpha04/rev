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

    if args.init {
        handle_init()?;
        return Ok(());
    }

    if args.uninit {
        handle_uninit()?;
        return Ok(());
    }

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

    #[cfg(target_os = "windows")]
    let child_pid = {
        use std::os::windows::process::CommandExt;
        const CREATE_SUSPENDED: u32 = 0x00000004;
        let resolved_program = resolve_program_path(program);
        let mut cmd = std::process::Command::new(&resolved_program);
        cmd.args(program_args);
        cmd.creation_flags(CREATE_SUSPENDED);
        let child = cmd.spawn()?;
        child.id()
    };

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let child_pid = {
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

pub fn handle_init() -> Result<(), RevError> {
    let mut found_env = false;

    // 1. Check Python virtual environments
    let venv_dirs = [".venv", "venv", "env"];
    for dir_name in &venv_dirs {
        let venv_path = std::path::Path::new(dir_name);
        if venv_path.is_dir() {
            #[cfg(target_os = "windows")]
            let script_dir = venv_path.join("Scripts");
            #[cfg(not(target_os = "windows"))]
            let script_dir = venv_path.join("bin");

            if script_dir.is_dir() {
                let current_exe = std::env::current_exe()?;
                
                // Shim python
                let python_names = ["python", "python3"];
                for name in &python_names {
                    #[cfg(target_os = "windows")]
                    let exe_filename = format!("{}.exe", name);
                    #[cfg(not(target_os = "windows"))]
                    let exe_filename = name.to_string();

                    let py_path = script_dir.join(&exe_filename);
                    if py_path.exists() && !py_path.is_symlink() {
                        #[cfg(target_os = "windows")]
                        let orig_filename = format!("{}_orig.exe", name);
                        #[cfg(not(target_os = "windows"))]
                        let orig_filename = format!("{}_orig", name);

                        let orig_path = script_dir.join(&orig_filename);
                        if !orig_path.exists() {
                            std::fs::rename(&py_path, &orig_path)?;
                            std::fs::copy(&current_exe, &py_path)?;
                            println!("rev: Shimmed {} in {}", name, dir_name);
                            found_env = true;
                        } else {
                            println!("rev: {} is already shimmed in {}", name, dir_name);
                            found_env = true;
                        }
                    }
                }
            }
        }
    }

    // 2. Check Node.js package.json
    let pkg_path = std::path::Path::new("package.json");
    if pkg_path.is_file() {
        let pkg_content = std::fs::read_to_string(pkg_path)?;
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&pkg_content) {
            if let Some(scripts) = json.get_mut("scripts").and_then(|s| s.as_object_mut()) {
                let mut modified = false;
                for (key, val) in scripts.iter_mut() {
                    if let Some(cmd) = val.as_str() {
                        let parts: Vec<&str> = cmd.split_whitespace().collect();
                        if let Some(&first) = parts.first() {
                            if (first == "node" || first == "python" || first == "python3" || first == "py") && first != "rev" {
                                let new_cmd = format!("rev {}", cmd);
                                *val = serde_json::Value::String(new_cmd);
                                println!("rev: Shimmed npm script '{}'", key);
                                modified = true;
                                found_env = true;
                            }
                        }
                    }
                }
                if modified {
                    let new_content = serde_json::to_string_pretty(&json)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    std::fs::write(pkg_path, new_content)?;
                }
            }
        }
    }

    if !found_env {
        println!("rev: No Python virtual environment (.venv, venv, env) or package.json found in the directory.");
    } else {
        println!("rev: Initialization complete! Project runs will now be tracked automatically.");
    }

    Ok(())
}

pub fn handle_uninit() -> Result<(), RevError> {
    let mut found_env = false;

    // 1. Restore Python shims
    let venv_dirs = [".venv", "venv", "env"];
    for dir_name in &venv_dirs {
        let venv_path = std::path::Path::new(dir_name);
        if venv_path.is_dir() {
            #[cfg(target_os = "windows")]
            let script_dir = venv_path.join("Scripts");
            #[cfg(not(target_os = "windows"))]
            let script_dir = venv_path.join("bin");

            if script_dir.is_dir() {
                let python_names = ["python", "python3"];
                for name in &python_names {
                    #[cfg(target_os = "windows")]
                    let orig_filename = format!("{}_orig.exe", name);
                    #[cfg(not(target_os = "windows"))]
                    let orig_filename = format!("{}_orig", name);

                    let orig_path = script_dir.join(&orig_filename);
                    if orig_path.exists() {
                        #[cfg(target_os = "windows")]
                        let exe_filename = format!("{}.exe", name);
                        #[cfg(not(target_os = "windows"))]
                        let exe_filename = name.to_string();

                        let py_path = script_dir.join(&exe_filename);
                        
                        let _ = std::fs::remove_file(&py_path);
                        std::fs::rename(&orig_path, &py_path)?;
                        println!("rev: Restored original {} in {}", name, dir_name);
                        found_env = true;
                    }
                }
            }
        }
    }

    // 2. Restore Node.js package.json
    let pkg_path = std::path::Path::new("package.json");
    if pkg_path.is_file() {
        let pkg_content = std::fs::read_to_string(pkg_path)?;
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&pkg_content) {
            if let Some(scripts) = json.get_mut("scripts").and_then(|s| s.as_object_mut()) {
                let mut modified = false;
                for (key, val) in scripts.iter_mut() {
                    if let Some(cmd) = val.as_str() {
                        if cmd.starts_with("rev ") {
                            let new_cmd = cmd.trim_start_matches("rev ").to_string();
                            *val = serde_json::Value::String(new_cmd);
                            println!("rev: Restored npm script '{}'", key);
                            modified = true;
                            found_env = true;
                        }
                    }
                }
                if modified {
                    let new_content = serde_json::to_string_pretty(&json)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    std::fs::write(pkg_path, new_content)?;
                }
            }
        }
    }

    if !found_env {
        println!("rev: No active shims or modified package.json found to restore.");
    } else {
        println!("rev: Uninitialization complete. Project tracking disabled.");
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn resolve_program_path(program: &str) -> String {
    let program_lower = program.to_lowercase();
    if program_lower == "python" || program_lower == "python3" || program_lower == "py" {
        // Check local virtual environments first
        let venv_candidates = [
            ".venv\\Scripts\\python.exe",
            "venv\\Scripts\\python.exe",
            "env\\Scripts\\python.exe",
        ];
        for candidate in &venv_candidates {
            if std::path::Path::new(candidate).exists() {
                return candidate.to_string();
            }
        }

        if program_lower != "py" && has_py_launcher() {
            return "py".to_string();
        }
    }
    program.to_string()
}

#[cfg(target_os = "windows")]
fn has_py_launcher() -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for path in std::env::split_paths(&path_var) {
            let py_exe = path.join("py.exe");
            if py_exe.exists() {
                return true;
            }
        }
    }
    false
}
