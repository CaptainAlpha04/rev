use rev_core::error::RevError;
use rev_replay::{NodeIntrospector, PythonIntrospector, RubyIntrospector, RuntimeIntrospector};

/// Detects which RuntimeIntrospector to use from the command
pub fn detect_runtime(command: &[String]) -> Result<Box<dyn RuntimeIntrospector>, RevError> {
    let binary = command.first().ok_or(RevError::NoCommand)?;
    let path = std::path::Path::new(binary);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(binary);
    let stem_lower = stem.to_lowercase();
    match stem_lower.as_str() {
        s if s.starts_with("python") || s == "py" => Ok(Box::new(PythonIntrospector::new())),
        "node" | "nodejs" | "npm" => Ok(Box::new(NodeIntrospector::new())),
        "ruby" => Ok(Box::new(RubyIntrospector::new())),
        other => Err(RevError::UnsupportedRuntime(other.to_string())),
    }
}
