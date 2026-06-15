use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "rev",
    version,
    about = "rev — Time-Traveler Runtime. Prefix any command with `rev` and get instant, zero-setup time-travel debugging."
)]
pub struct CliArgs {
    /// Initialize rev in the current project (shims local venv python and package.json scripts)
    #[arg(long)]
    pub init: bool,

    /// Uninitialize rev from the current project (restores shims and package.json scripts)
    #[arg(long)]
    pub uninit: bool,

    /// The interpreter and program to run (e.g., "python main.py")
    #[arg(required_unless_present_any = ["replay", "export", "init", "uninit"])]
    pub runtime: Option<String>,

    /// Arguments passed through to the program
    #[arg(last = true)]
    pub passthrough_args: Vec<String>,

    /// Where to save the .rev-trace file
    #[arg(short, long, default_value = "~/.rev/traces")]
    pub output: String,

    /// Snapshot interval in steps
    #[arg(short, long, default_value_t = 100)]
    pub step_size: u64,

    /// Print recording stats during execution
    #[arg(short, long)]
    pub verbose: bool,

    /// Record only, don't open TUI on crash
    #[arg(long)]
    pub no_tui: bool,

    /// Open TUI directly on an existing .rev-trace file
    #[arg(long, value_name = "TRACE")]
    pub replay: Option<PathBuf>,

    /// Export state at step N to stdout as JSON (no TUI). Usage: --export <TRACE> <STEP>
    #[arg(long, num_args = 2, value_names = ["TRACE", "STEP"])]
    pub export: Option<Vec<String>>,
}

impl CliArgs {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
