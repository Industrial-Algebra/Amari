// SPDX-License-Identifier: MIT OR Apache-2.0

//! Private fixed-launch process supervisor for CLI probe isolation.

use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use crate::DiscoveryResult;

// This private foundation is wired into command dispatch by Task 21C.
#[allow(dead_code)]
trait WorkerLauncher {
    fn command(&self) -> DiscoveryResult<Command>;
}

// This private foundation is wired into command dispatch by Task 21C.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) struct ProductionWorkerLauncher;

impl WorkerLauncher for ProductionWorkerLauncher {
    fn command(&self) -> DiscoveryResult<Command> {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("__probe-worker");
        Ok(command)
    }
}

// This private foundation is wired into command dispatch by Task 21C.
#[allow(dead_code)]
fn spawn_restricted(launcher: &impl WorkerLauncher) -> DiscoveryResult<Child> {
    let mut command = launcher.command()?;
    command
        .env_clear()
        .current_dir(neutral_working_directory())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command.spawn()?)
}

fn neutral_working_directory() -> PathBuf {
    let temporary = std::env::temp_dir();
    temporary.canonicalize().unwrap_or(temporary)
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct TestWorkerLauncher {
    fixture: PathBuf,
    mode: String,
    poison_context: Option<PathBuf>,
}

#[cfg(test)]
impl TestWorkerLauncher {
    fn new(fixture: PathBuf, mode: &str) -> Self {
        Self {
            fixture,
            mode: mode.to_owned(),
            poison_context: None,
        }
    }

    fn with_poison_context(mut self, path: &str) -> Self {
        self.poison_context = Some(PathBuf::from(path));
        self
    }
}

#[cfg(test)]
impl WorkerLauncher for TestWorkerLauncher {
    fn command(&self) -> DiscoveryResult<Command> {
        let mut command = Command::new("python3");
        command.arg(&self.fixture).arg(&self.mode);
        if let Some(poison) = &self.poison_context {
            command
                .current_dir(poison)
                .env("AMARI_PROJECT_ROOT", poison)
                .env("AMARI_PROJECT_SECRET", "must-not-reach-worker");
        }
        Ok(command)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use super::*;

    const PROJECT_POISON: &str = "/private/project/never-pass-this";

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe-test-worker.py")
    }

    #[test]
    fn production_command_and_arguments_are_fixed_internally() {
        let launcher = ProductionWorkerLauncher;
        let command = launcher.command().unwrap();
        let expected_executable = std::env::current_exe().unwrap();

        assert_eq!(command.get_program(), expected_executable.as_os_str());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("__probe-worker")]
        );
        assert_eq!(command.get_envs().count(), 0);
        assert!(command.get_current_dir().is_none());
    }

    #[test]
    fn restricted_launch_clears_environment_and_uses_neutral_cwd() {
        let launcher =
            TestWorkerLauncher::new(fixture_path(), "context").with_poison_context(PROJECT_POISON);
        let neutral = neutral_working_directory();
        let output = spawn_restricted(&launcher)
            .unwrap()
            .wait_with_output()
            .unwrap();

        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let context: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(context["argv"], serde_json::json!(["context"]));
        assert_eq!(context["cwd"], neutral.to_string_lossy().as_ref());
        let environment = context["environment"].as_object().unwrap();
        assert!(
            environment.keys().all(|key| key == "LC_CTYPE"),
            "only CPython's process-local locale normalization is permitted: {environment:?}"
        );
        let encoded = serde_json::to_string(&context).unwrap();
        assert!(!encoded.contains(PROJECT_POISON));
    }

    #[test]
    fn production_launcher_has_no_configuration_or_request_inputs() {
        fn assert_unit<T>(_: T) {}
        assert_unit(ProductionWorkerLauncher);

        let command = ProductionWorkerLauncher.command().unwrap();
        let encoded = format!("{command:?}");
        assert!(!encoded.contains(PROJECT_POISON));
        assert!(!encoded.contains("sh -c"));
        assert!(!encoded.contains("cmd /C"));
    }
}
