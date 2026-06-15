pub mod fast_forward;
pub mod headless;
pub mod runtimes;

use rev_core::error::RevError;
use rev_core::types::{ProgramState, StackFrame, Variable};
use rev_delta::DeltaEngine;
use rev_recorder::TraceReader;
use std::path::Path;

pub use headless::HeadlessProcess;
pub use runtimes::python::PythonIntrospector;
pub use runtimes::node::NodeIntrospector;
pub use runtimes::ruby::RubyIntrospector;

pub trait RuntimeIntrospector: Send {
    /// Name of the runtime this handles (e.g., "python3")
    fn runtime_name(&self) -> &str;

    /// Spawn the interpreter in replay mode with syscall interception
    fn spawn_headless(&self, program: &Path, args: &[String]) -> Result<HeadlessProcess, RevError>;

    /// Extract variable values from the running headless process at current state
    fn extract_variables(&self, proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError>;

    /// Extract the current call stack
    fn extract_call_stack(&self, proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError>;
}

#[allow(dead_code)]
pub struct ReplayEngine {
    trace_path: std::path::PathBuf,
    reader: TraceReader,
    delta: DeltaEngine,
    runtime: Box<dyn RuntimeIntrospector>,
}

impl ReplayEngine {
    pub fn new(trace_path: &Path, runtime: Box<dyn RuntimeIntrospector>) -> Result<Self, RevError> {
        let reader = TraceReader::new(trace_path)?;
        let pid = reader.header.pid;
        let delta = DeltaEngine::new(pid)?;

        Ok(Self {
            trace_path: trace_path.to_path_buf(),
            reader,
            delta,
            runtime,
        })
    }

    /// Reconstruct state at given step. Returns variable values and call stack.
    pub fn state_at(&mut self, step: u64) -> Result<ProgramState, RevError> {
        // Read all events from the trace
        let events = self.reader.read_all_events()?;
        #[allow(unused_variables)]
        let target_event =
            events
                .iter()
                .find(|e| e.id == step)
                .ok_or_else(|| RevError::ReplayFailed {
                    step,
                    reason: format!("Event step {} not found in trace", step),
                })?;

        // Resolve the command/program to run from the trace configuration or args
        // Under Linux, we spawn the headless interpreter
        #[cfg(target_os = "linux")]
        {
            let program = Path::new(self.runtime.runtime_name());
            let mut proc = self.runtime.spawn_headless(program, &[])?;
            fast_forward::fast_forward_process(&mut proc, &events, step)?;

            let variables = self.runtime.extract_variables(&proc)?;
            let call_stack = self.runtime.extract_call_stack(&proc)?;

            Ok(ProgramState {
                step,
                timestamp_ns: target_event.timestamp_ns,
                variables,
                call_stack,
                last_event: target_event.clone(),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(RevError::UnsupportedPlatform(
                "State reconstruction is only supported on Linux".to_string(),
            ))
        }
    }

    /// Total number of steps recorded.
    pub fn step_count(&self) -> u64 {
        // We can create a new reader temporarily to parse the trace and count events
        if let Ok(mut r) = TraceReader::new(&self.trace_path) {
            if let Ok(events) = r.read_all_events() {
                let events: Vec<rev_core::types::SyscallEvent> = events;
                return events.len() as u64;
            }
        }
        0
    }

    /// Human-readable summary of what happened at a given step.
    pub fn event_summary(&self, step: u64) -> Option<String> {
        if let Ok(mut r) = TraceReader::new(&self.trace_path) {
            if let Ok(events) = r.read_all_events() {
                let events: Vec<rev_core::types::SyscallEvent> = events;
                if let Some(event) = events.iter().find(|e| e.id == step) {
                    return Some(format!("Step {}: Syscall {:?}", step, event.syscall));
                }
            }
        }
        None
    }
}

