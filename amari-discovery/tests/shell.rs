// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    fs,
    io::{Read, Write},
    process::{Command as ProcessCommand, Stdio},
};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn rust_project() -> TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("Cargo.toml"),
        r#"[package]
name = "shell-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.23.0"
"#,
    )
    .unwrap();
    fs::write(
        project.path().join("src/lib.rs"),
        "use amari_core::Multivector;\npub fn scalar() -> Multivector<3,0,0> { Multivector::scalar(1.0) }\n",
    )
    .unwrap();
    project
}

fn npm_project() -> TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::write(
        project.path().join("package.json"),
        r#"{"name":"shell-npm","version":"0.1.0","dependencies":{"@justinelliottcobb/amari-wasm":"0.23.0"}}"#,
    )
    .unwrap();
    project
}

fn saved_recommendation(project: &TempDir) -> (TempDir, String, std::path::PathBuf) {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args([
            "recommend",
            project.path().to_str().unwrap(),
            "--goal",
            "compute a geometric product",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let recommendation: Value = serde_json::from_slice(&output).unwrap();
    let candidate = recommendation["data"]["data"]["preferred"]["capability_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recommendation.json");
    fs::write(&path, output).unwrap();
    (directory, candidate, path)
}

#[test]
fn session_project_defaults_inspect_recommend_and_plan_while_explicit_paths_override() {
    let rust = rust_project();
    let npm = npm_project();
    let (_artifact, candidate, recommendation) = saved_recommendation(&rust);
    let input = format!(
        "inspect\ninspect {}\nrecommend --goal \"compute a geometric product\"\nplan {} --recommendation {}\nexit\n",
        npm.path().display(),
        candidate,
        recommendation.display(),
    );

    Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--project", rust.path().to_str().unwrap()])
        .write_stdin(input)
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Rust/Cargo project"))
        .stdout(predicate::str::contains("npm/TypeScript project"))
        .stdout(predicate::str::contains("Recommendation for:"))
        .stdout(predicate::str::contains("Plan for:"));
}

#[test]
fn shell_delegates_help_capabilities_discovery_and_probe_authority() {
    Command::cargo_bin("amari")
        .unwrap()
        .arg("shell")
        .write_stdin(
            "help\ncapabilities\ndiscover search tropical\ndiscover detail amari:amari-tropical:sequence:viterbi\nprobe list\nexit\n",
        )
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Shell commands:"))
        .stdout(predicate::str::contains("Amari Discovery"))
        .stdout(predicate::str::contains("Registered probes:"))
        .stdout(predicate::str::contains("amari.discovery/v1"));
}

#[test]
fn nonexistent_session_project_returns_the_inspection_error_and_stable_exit_code() {
    let missing = tempfile::tempdir().unwrap().path().join("missing");
    Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--project", missing.to_str().unwrap()])
        .write_stdin("inspect\n")
        .assert()
        .code(4)
        .stderr(predicate::str::contains("inspection_failure"));
}

#[test]
fn session_project_is_revalidated_after_the_shell_starts() {
    let project = rust_project();
    let binary = Command::cargo_bin("amari")
        .unwrap()
        .get_program()
        .to_owned();
    let mut child = ProcessCommand::new(binary)
        .args(["shell", "--project", project.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    stdin.write_all(b"capabilities\n").unwrap();
    stdin.flush().unwrap();

    let mut observed = Vec::new();
    while !observed.ends_with(b"amari> ") {
        let mut byte = [0_u8; 1];
        assert_eq!(stdout.read(&mut byte).unwrap(), 1);
        observed.push(byte[0]);
    }
    drop(project);
    stdin.write_all(b"inspect\n").unwrap();
    drop(stdin);

    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(4));
    assert!(stderr.contains("inspection_failure"), "{stderr}");
}
