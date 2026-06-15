use rev_cli::args::CliArgs;
use rev_cli::orchestrator::run_orchestrator;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_integration_recording_pipeline() {
    let temp_dir =
        std::env::temp_dir().join(format!("rev_integration_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let args = CliArgs {
        runtime: Some("python".to_string()),
        passthrough_args: vec!["mock_program.py".to_string()],
        output: temp_dir.to_string_lossy().into_owned(),
        step_size: 100,
        verbose: true,
        no_tui: true,
        replay: None,
        export: None,
    };

    // Run recording loop
    run_orchestrator(args).unwrap();

    // Find created trace files
    let mut files: Vec<PathBuf> = fs::read_dir(&temp_dir)
        .unwrap()
        .map(|res| res.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rev-trace"))
        .collect();

    assert_eq!(
        files.len(),
        1,
        "Expected exactly one trace file to be created"
    );
    let trace_path = files.pop().unwrap();
    let filename = trace_path.file_name().unwrap().to_string_lossy();

    // Extract PID from filename: <pid>_<timestamp>.rev-trace
    let parts: Vec<&str> = filename.split('_').collect();
    let pid_str = parts[0];
    let pid: u32 = pid_str.parse().unwrap();

    // Verify SQLite database exists under ~/.rev/deltas/<pid>.db
    let mut db_path = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
        PathBuf::from(userprofile)
    } else {
        PathBuf::from(".")
    };
    db_path.push(".rev");
    db_path.push("deltas");
    db_path.push(format!("{}.db", pid));

    assert!(
        db_path.exists(),
        "Expected SQLite database at {:?}",
        db_path
    );

    // Verify SQLite database contents
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM deltas", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();
    assert!(
        count > 0,
        "Expected at least one delta node in the SQLite database"
    );

    // Clean up
    let _ = fs::remove_file(&trace_path);
    let _ = fs::remove_dir(&temp_dir);
    let _ = fs::remove_file(&db_path);
}
