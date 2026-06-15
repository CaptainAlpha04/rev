use rev_cli::args::CliArgs;
use rev_cli::orchestrator::run_orchestrator;

fn main() {
    if let Some(args) = check_and_handle_shim() {
        if let Err(e) = run_orchestrator(args) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let args = CliArgs::parse_args();
    if let Err(e) = run_orchestrator(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn check_and_handle_shim() -> Option<CliArgs> {
    let args: Vec<String> = std::env::args().collect();
    if args.is_empty() {
        return None;
    }

    let exe_path = std::path::Path::new(&args[0]);
    let exe_name = exe_path.file_stem()?.to_str()?.to_lowercase();

    if exe_name == "python" || exe_name == "python3" {
        let dir = exe_path.parent()?;
        
        #[cfg(target_os = "windows")]
        let orig_name = "python_orig.exe";
        #[cfg(not(target_os = "windows"))]
        let orig_name = "python_orig";

        let orig_exe = dir.join(orig_name);
        if orig_exe.exists() {
            let runtime = orig_exe.to_string_lossy().to_string();
            let passthrough_args = args[1..].to_vec();

            return Some(CliArgs {
                init: false,
                uninit: false,
                runtime: Some(runtime),
                passthrough_args,
                output: "~/.rev/traces".to_string(),
                step_size: 100,
                verbose: false,
                no_tui: false,
                replay: None,
                export: None,
            });
        }
    }

    None
}
