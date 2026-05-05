use std::path::PathBuf;
use std::process::Command;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/p0_monolith_demo.json")
}

fn target_edge_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/external-input/target/edge-discovery-snapshot.json")
}

fn run_app(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_topology-app"))
        .args(args)
        .output()
        .expect("failed to run topology-app")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "process failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout_text(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be valid utf-8")
}

#[test]
fn smoke_demo_mode_builds_minimal_closure() {
    let output = run_app(&[]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("dayu-topology monolith started"));
    assert!(stdout.contains("ingest_id=demo-ingest-1"));
    assert!(stdout.contains("host=demo-node"));
    assert!(stdout.contains("network=10.42.0.0/24"));
    assert!(stdout.contains("ip=10.42.0.12"));
    assert!(stdout.contains("responsibilities=alice:Owner"));
}

#[test]
fn smoke_file_mode_builds_minimal_closure_from_fixture() {
    let fixture = fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["file", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("ingest_id=demo-ingest-1"));
    assert!(stdout.contains("host=demo-node"));
    assert!(stdout.contains("network=10.42.0.0/24"));
    assert!(stdout.contains("ip=10.42.0.12"));
    assert!(stdout.contains("responsibilities=alice:Owner"));
}

#[test]
fn smoke_file_mode_accepts_target_dayu_input_envelope() {
    let fixture = target_edge_fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["file", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("ingest_id=dayu.in.edge.v1:warp-insight:agent-office-01:tenant-demo:office:edge-snap-20260426-office-01"));
    assert!(stdout.contains("host=office-build-01"));
    assert!(stdout.contains("network=192.168.10.0/24"));
    assert!(stdout.contains("ip=192.168.10.52"));
}

#[test]
fn smoke_postgres_mock_mode_uses_same_ingest_query_path() {
    let output = run_app(&["postgres-mock"]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("dayu-topology monolith started"));
    assert!(stdout.contains("ingest_id=demo-ingest-1"));
    assert!(stdout.contains("host=demo-node"));
    assert!(stdout.contains("network=10.42.0.0/24"));
    assert!(stdout.contains("ip=10.42.0.12"));
    assert!(stdout.contains("responsibilities=alice:Owner"));
}
