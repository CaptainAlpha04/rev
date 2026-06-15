use crate::{HeadlessProcess, RuntimeIntrospector};
use rev_core::error::RevError;
use rev_core::types::{StackFrame, Variable};
use std::path::Path;

#[derive(Default)]
pub struct NodeIntrospector;

impl NodeIntrospector {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "linux")]
impl RuntimeIntrospector for NodeIntrospector {
    fn runtime_name(&self) -> &str {
        "node"
    }

    fn spawn_headless(&self, program: &Path, args: &[String]) -> Result<HeadlessProcess, RevError> {
        crate::headless::spawn_headless(program, args)
    }

    fn extract_variables(&self, proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError> {
        let _node_base = find_node_base(proc.pid)?;
        Ok(Vec::new())
    }

    fn extract_call_stack(&self, proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError> {
        let _node_base = find_node_base(proc.pid)?;
        Ok(Vec::new())
    }
}

#[cfg(target_os = "linux")]
fn find_node_base(pid: u32) -> Result<(String, u64), RevError> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let maps_path = format!("/proc/{}/maps", pid);
    let file = File::open(maps_path)?;
    let reader = BufReader::new(file);

    for line_result in reader.lines() {
        let line = line_result?;
        if line.contains("/node") || line.contains("libnode") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }
            let addr_part = parts[0];
            let mut addrs = addr_part.split('-');
            let start_str = addrs.next().ok_or_else(|| RevError::TraceCorrupted {
                offset: 0,
                reason: "invalid maps".to_string(),
            })?;
            let start = u64::from_str_radix(start_str, 16).map_err(|e| {
                RevError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            })?;
            let path = parts[5..].join(" ");
            return Ok((path, start));
        }
    }
    Err(RevError::UnsupportedRuntime(
        "node binary/library not found in maps".to_string(),
    ))
}

#[cfg(not(target_os = "linux"))]
impl RuntimeIntrospector for NodeIntrospector {
    fn runtime_name(&self) -> &str {
        "node"
    }

    fn spawn_headless(
        &self,
        _program: &Path,
        _args: &[String],
    ) -> Result<HeadlessProcess, RevError> {
        Err(RevError::UnsupportedPlatform(
            "Node.js Introspection is only supported on Linux".to_string(),
        ))
    }

    fn extract_variables(&self, _proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError> {
        Err(RevError::UnsupportedPlatform(
            "Node.js Introspection is only supported on Linux".to_string(),
        ))
    }

    fn extract_call_stack(&self, _proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError> {
        Err(RevError::UnsupportedPlatform(
            "Node.js Introspection is only supported on Linux".to_string(),
        ))
    }
}
