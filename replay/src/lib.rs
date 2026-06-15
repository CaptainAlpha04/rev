use rev_core::error::RevError;
use rev_core::types::{ProgramState, StackFrame, Variable};
use rev_delta::DeltaEngine;
use std::path::Path;

pub struct TraceReader;
pub struct HeadlessProcess;

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

pub struct ReplayEngine {
    _trace: TraceReader,
    _delta: DeltaEngine,
    _runtime: Box<dyn RuntimeIntrospector>,
}

impl ReplayEngine {
    pub fn new(
        _trace_path: &Path,
        _runtime: Box<dyn RuntimeIntrospector>,
    ) -> Result<Self, RevError> {
        todo!("ReplayEngine::new is not implemented yet")
    }

    /// Reconstruct state at given step. Returns variable values and call stack.
    pub fn state_at(&mut self, _step: u64) -> Result<ProgramState, RevError> {
        todo!("ReplayEngine::state_at is not implemented yet")
    }

    /// Total number of steps recorded.
    pub fn step_count(&self) -> u64 {
        todo!("ReplayEngine::step_count is not implemented yet")
    }

    /// Human-readable summary of what happened at a given step.
    pub fn event_summary(&self, _step: u64) -> Option<String> {
        todo!("ReplayEngine::event_summary is not implemented yet")
    }
}

#[derive(Default)]
pub struct PythonIntrospector;
impl PythonIntrospector {
    pub fn new() -> Self {
        Self
    }
}
impl RuntimeIntrospector for PythonIntrospector {
    fn runtime_name(&self) -> &str {
        "python3"
    }
    fn spawn_headless(
        &self,
        _program: &Path,
        _args: &[String],
    ) -> Result<HeadlessProcess, RevError> {
        todo!("PythonIntrospector::spawn_headless")
    }
    fn extract_variables(&self, _proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError> {
        todo!("PythonIntrospector::extract_variables")
    }
    fn extract_call_stack(&self, _proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError> {
        todo!("PythonIntrospector::extract_call_stack")
    }
}

#[derive(Default)]
pub struct NodeIntrospector;
impl NodeIntrospector {
    pub fn new() -> Self {
        Self
    }
}
impl RuntimeIntrospector for NodeIntrospector {
    fn runtime_name(&self) -> &str {
        "node"
    }
    fn spawn_headless(
        &self,
        _program: &Path,
        _args: &[String],
    ) -> Result<HeadlessProcess, RevError> {
        todo!("NodeIntrospector::spawn_headless")
    }
    fn extract_variables(&self, _proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError> {
        todo!("NodeIntrospector::extract_variables")
    }
    fn extract_call_stack(&self, _proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError> {
        todo!("NodeIntrospector::extract_call_stack")
    }
}

#[derive(Default)]
pub struct RubyIntrospector;
impl RubyIntrospector {
    pub fn new() -> Self {
        Self
    }
}
impl RuntimeIntrospector for RubyIntrospector {
    fn runtime_name(&self) -> &str {
        "ruby"
    }
    fn spawn_headless(
        &self,
        _program: &Path,
        _args: &[String],
    ) -> Result<HeadlessProcess, RevError> {
        todo!("RubyIntrospector::spawn_headless")
    }
    fn extract_variables(&self, _proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError> {
        todo!("RubyIntrospector::extract_variables")
    }
    fn extract_call_stack(&self, _proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError> {
        todo!("RubyIntrospector::extract_call_stack")
    }
}
