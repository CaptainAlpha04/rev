use rev_core::error::RevError;
use rev_replay::{NodeIntrospector, PythonIntrospector, RubyIntrospector, RuntimeIntrospector};

/// Detects which RuntimeIntrospector to use from the command
pub fn detect_runtime(command: &[String]) -> Result<Box<dyn RuntimeIntrospector>, RevError> {
    let binary = command.first().ok_or(RevError::NoCommand)?;
    match binary.as_str() {
        s if s.starts_with("python") => Ok(Box::new(PythonIntrospector::new())),
        "node" | "nodejs" => Ok(Box::new(NodeIntrospector::new())),
        "ruby" => Ok(Box::new(RubyIntrospector::new())),
        other => Err(RevError::UnsupportedRuntime(other.to_string())),
    }
}
