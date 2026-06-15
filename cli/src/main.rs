use rev_cli::args::CliArgs;
use rev_cli::orchestrator::run_orchestrator;

fn main() {
    let args = CliArgs::parse_args();
    if let Err(e) = run_orchestrator(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
