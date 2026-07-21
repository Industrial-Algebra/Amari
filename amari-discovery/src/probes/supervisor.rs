// SPDX-License-Identifier: MIT OR Apache-2.0

//! Private fixed-launch process supervisor for CLI probe isolation.

use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{
    worker::{self, WorkerRequest, WorkerResponse},
    ProbeIsolation,
};
use crate::{
    DiscoveryError, DiscoveryResult, ProbeEngineLimits, ProbeExecution, ProbeId, Provenance,
    ResourceLimits,
};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SupervisorIoLimits {
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}

#[derive(Debug)]
struct CapturedOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
enum StreamKind {
    Stdout,
    Stderr,
}

impl StreamKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Debug)]
struct StreamLimit {
    kind: StreamKind,
    observed: usize,
    maximum: usize,
}

#[derive(Debug)]
enum DrainOutcome {
    Complete(Vec<u8>),
    LimitExceeded(StreamLimit),
    Failed(io::Error),
}

trait WorkerLauncher {
    fn command(&self) -> DiscoveryResult<Command>;
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProductionWorkerLauncher;

impl WorkerLauncher for ProductionWorkerLauncher {
    fn command(&self) -> DiscoveryResult<Command> {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("__probe-worker");
        Ok(command)
    }
}

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

pub(super) fn execute_isolated(
    probe_id: &ProbeId,
    input: &serde_json::Value,
    limits: ProbeEngineLimits,
    provenance: Provenance,
) -> DiscoveryResult<ProbeExecution> {
    let defaults = ResourceLimits::default();
    let response = supervise_worker(
        &ProductionWorkerLauncher,
        &WorkerRequest {
            probe_id: probe_id.clone(),
            input: input.clone(),
            limits,
            provenance,
        },
        SupervisorIoLimits {
            max_stdout_bytes: 2 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        },
        Duration::from_millis(defaults.probe_timeout_millis),
    )?;
    Ok(response.execution)
}

fn supervise_worker(
    launcher: &impl WorkerLauncher,
    request: &WorkerRequest,
    io_limits: SupervisorIoLimits,
    deadline: Duration,
) -> DiscoveryResult<WorkerResponse> {
    if deadline.is_zero() {
        return Err(DiscoveryError::InvalidInput(
            "probe worker deadline must be greater than zero".to_owned(),
        ));
    }
    if io_limits.max_stdout_bytes == 0 || io_limits.max_stderr_bytes == 0 {
        return Err(DiscoveryError::InvalidInput(
            "probe worker stream limits must be greater than zero".to_owned(),
        ));
    }

    let mut child = spawn_restricted(launcher)?;
    if let Err(error) = send_worker_request(&mut child, request) {
        terminate_and_wait(&mut child)?;
        return Err(error);
    }
    let captured = capture_bounded(child, io_limits, deadline)?;
    map_worker_outcome(captured, &request.provenance)
}

fn send_worker_request(child: &mut Child, request: &WorkerRequest) -> DiscoveryResult<()> {
    let frame = worker::encode_request_frame(request)?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        DiscoveryError::Internal("probe worker stdin pipe is unavailable".to_owned())
    })?;
    stdin.write_all(&frame)?;
    stdin.flush()?;
    drop(stdin);
    Ok(())
}

fn map_worker_outcome(
    captured: CapturedOutput,
    expected_provenance: &Provenance,
) -> DiscoveryResult<WorkerResponse> {
    if !captured.status.success() {
        if let Some(code) = captured.status.code() {
            return Err(DiscoveryError::ProbeWorkerExited { code });
        }
        return Err(DiscoveryError::ProbeWorkerCrashed {
            signal: exit_signal(&captured.status),
        });
    }

    if !captured.stderr.is_empty() {
        return Err(DiscoveryError::ProbeWorkerProtocol(
            "successful worker emitted unexpected diagnostics".to_owned(),
        ));
    }
    let mut response = decode_worker_response(&captured.stdout).map_err(|error| {
        DiscoveryError::ProbeWorkerProtocol(format!("invalid framed response ({})", error.kind()))
    })?;
    if &response.provenance != expected_provenance {
        return Err(DiscoveryError::ProbeWorkerProtocol(
            "worker response provenance differs from the request".to_owned(),
        ));
    }
    response.execution.isolation = ProbeIsolation::Process;
    Ok(response)
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

fn capture_bounded(
    mut child: Child,
    limits: SupervisorIoLimits,
    deadline: Duration,
) -> DiscoveryResult<CapturedOutput> {
    if deadline.is_zero() {
        terminate_and_wait(&mut child)?;
        return Err(DiscoveryError::InvalidInput(
            "probe worker deadline must be greater than zero".to_owned(),
        ));
    }
    let started = Instant::now();
    let stdout = child.stdout.take().ok_or_else(|| {
        DiscoveryError::Internal("probe worker stdout pipe is unavailable".to_owned())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        DiscoveryError::Internal("probe worker stderr pipe is unavailable".to_owned())
    })?;
    drop(child.stdin.take());

    let (signals, receiver) = mpsc::channel();
    let stdout_drain = spawn_bounded_drain(
        stdout,
        limits.max_stdout_bytes,
        StreamKind::Stdout,
        signals.clone(),
    );
    let stderr_drain =
        spawn_bounded_drain(stderr, limits.max_stderr_bytes, StreamKind::Stderr, signals);

    let status = loop {
        if let Ok(limit) = receiver.try_recv() {
            terminate_and_wait(&mut child)?;
            let _ = join_drain(stdout_drain);
            let _ = join_drain(stderr_drain);
            return Err(limit_error(&limit));
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= deadline {
            terminate_and_wait(&mut child)?;
            let _ = join_drain(stdout_drain);
            let _ = join_drain(stderr_drain);
            return Err(DiscoveryError::LimitExceeded(format!(
                "probe worker wall-clock deadline of {} milliseconds exceeded",
                deadline.as_millis()
            )));
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    };

    let stdout = finish_drain(join_drain(stdout_drain)?)?;
    let stderr = finish_drain(join_drain(stderr_drain)?)?;
    Ok(CapturedOutput {
        status,
        stdout,
        stderr,
    })
}

fn spawn_bounded_drain(
    mut reader: impl Read + Send + 'static,
    maximum: usize,
    kind: StreamKind,
    signals: Sender<StreamLimit>,
) -> JoinHandle<DrainOutcome> {
    thread::spawn(move || {
        let mut output = Vec::with_capacity(maximum.min(64 * 1024));
        let mut buffer = [0_u8; 8192];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(error) => return DrainOutcome::Failed(error),
            };
            if read == 0 {
                return DrainOutcome::Complete(output);
            }
            let observed = match output.len().checked_add(read) {
                Some(observed) => observed,
                None => usize::MAX,
            };
            if observed > maximum {
                let _ = signals.send(StreamLimit {
                    kind,
                    observed,
                    maximum,
                });
                return DrainOutcome::LimitExceeded(StreamLimit {
                    kind,
                    observed,
                    maximum,
                });
            }
            output.extend_from_slice(&buffer[..read]);
        }
    })
}

fn join_drain(handle: JoinHandle<DrainOutcome>) -> DiscoveryResult<DrainOutcome> {
    handle
        .join()
        .map_err(|_| DiscoveryError::Internal("probe worker drain thread panicked".to_owned()))
}

fn finish_drain(outcome: DrainOutcome) -> DiscoveryResult<Vec<u8>> {
    match outcome {
        DrainOutcome::Complete(output) => Ok(output),
        DrainOutcome::LimitExceeded(limit) => Err(limit_error(&limit)),
        DrainOutcome::Failed(error) => Err(DiscoveryError::Io(error)),
    }
}

fn limit_error(limit: &StreamLimit) -> DiscoveryError {
    DiscoveryError::LimitExceeded(format!(
        "probe worker {} bytes {} exceeds limit {}",
        limit.kind.name(),
        limit.observed,
        limit.maximum
    ))
}

fn terminate_and_wait(child: &mut Child) -> DiscoveryResult<()> {
    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(DiscoveryError::Io(error)),
    }
    child.wait()?;
    Ok(())
}

fn decode_worker_response(bytes: &[u8]) -> DiscoveryResult<WorkerResponse> {
    worker::decode_response_frame(bytes)
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct TestWorkerLauncher {
    fixture: PathBuf,
    mode: String,
    arguments: Vec<PathBuf>,
    poison_context: Option<PathBuf>,
}

#[cfg(test)]
impl TestWorkerLauncher {
    fn new(fixture: PathBuf, mode: &str) -> Self {
        Self {
            fixture,
            mode: mode.to_owned(),
            arguments: Vec::new(),
            poison_context: None,
        }
    }

    fn with_poison_context(mut self, path: &str) -> Self {
        self.poison_context = Some(PathBuf::from(path));
        self
    }

    fn with_arguments(mut self, arguments: impl IntoIterator<Item = PathBuf>) -> Self {
        self.arguments.extend(arguments);
        self
    }
}

#[cfg(test)]
impl WorkerLauncher for TestWorkerLauncher {
    fn command(&self) -> DiscoveryResult<Command> {
        let mut command = Command::new("python3");
        command
            .arg(&self.fixture)
            .arg(&self.mode)
            .args(&self.arguments);
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
    use std::{
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use serde_json::Value;

    use super::*;

    const PROJECT_POISON: &str = "/private/project/never-pass-this";

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe-test-worker.py")
    }

    fn fixture_provenance() -> crate::Provenance {
        crate::Provenance {
            tool_version: "fixture".to_owned(),
            catalog: crate::CatalogIdentity {
                version: "fixture".to_owned(),
                hash: "fixture-catalog".to_owned(),
            },
            compatibility: crate::Compatibility {
                status: "compatible".to_owned(),
                reasons: Vec::new(),
            },
            replay: crate::ReplayMetadata {
                replayable: true,
                required_hashes: Vec::new(),
                reasons: Vec::new(),
            },
            project_hash: None,
            input_hash: Some("fixture-input".to_owned()),
            seed: None,
        }
    }

    fn fixture_request() -> worker::WorkerRequest {
        worker::WorkerRequest {
            probe_id: "amari-probe:tropical:viterbi:v1".parse().unwrap(),
            input: serde_json::json!({"fixture": true}),
            limits: super::super::ProbeEngineLimits::default(),
            provenance: fixture_provenance(),
        }
    }

    fn run_fixture(mode: &str) -> DiscoveryResult<WorkerResponse> {
        supervise_worker(
            &TestWorkerLauncher::new(fixture_path(), mode),
            &fixture_request(),
            SupervisorIoLimits {
                max_stdout_bytes: 64 * 1024,
                max_stderr_bytes: 64 * 1024,
            },
            Duration::from_secs(5),
        )
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

    #[test]
    fn bounded_capture_drains_stderr_and_decodes_stdout_without_deadlock() {
        let launcher = TestWorkerLauncher::new(fixture_path(), "simultaneous");
        let captured = capture_bounded(
            spawn_restricted(&launcher).unwrap(),
            SupervisorIoLimits {
                max_stdout_bytes: 64 * 1024,
                max_stderr_bytes: 512 * 1024,
            },
            Duration::from_secs(5),
        )
        .unwrap();

        assert!(captured.status.success());
        assert_eq!(captured.stderr.len(), 256 * 1024);
        let response = decode_worker_response(&captured.stdout).unwrap();
        assert_eq!(
            response.execution.output,
            serde_json::json!({"fixture": true})
        );
        assert_eq!(
            response.provenance.input_hash.as_deref(),
            Some("fixture-input")
        );
    }

    #[test]
    fn stdout_flood_exceeding_cap_terminates_the_worker() {
        let started = Instant::now();
        let launcher = TestWorkerLauncher::new(fixture_path(), "flood-stdout");
        let error = capture_bounded(
            spawn_restricted(&launcher).unwrap(),
            SupervisorIoLimits {
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            },
            Duration::from_secs(5),
        )
        .unwrap_err();

        assert!(
            matches!(error, crate::DiscoveryError::LimitExceeded(message) if message.contains("stdout"))
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn stderr_flood_exceeding_cap_terminates_the_worker() {
        let started = Instant::now();
        let launcher = TestWorkerLauncher::new(fixture_path(), "flood-stderr");
        let error = capture_bounded(
            spawn_restricted(&launcher).unwrap(),
            SupervisorIoLimits {
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            },
            Duration::from_secs(5),
        )
        .unwrap_err();

        assert!(
            matches!(error, crate::DiscoveryError::LimitExceeded(message) if message.contains("stderr"))
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn valid_worker_frame_decodes_with_empty_diagnostics() {
        let launcher = TestWorkerLauncher::new(fixture_path(), "valid");
        let captured = capture_bounded(
            spawn_restricted(&launcher).unwrap(),
            SupervisorIoLimits {
                max_stdout_bytes: 64 * 1024,
                max_stderr_bytes: 64 * 1024,
            },
            Duration::from_secs(5),
        )
        .unwrap();

        assert!(captured.stderr.is_empty());
        let response = decode_worker_response(&captured.stdout).unwrap();
        assert_eq!(
            response.execution.probe_id.to_string(),
            "amari-probe:tropical:viterbi:v1"
        );
    }

    #[test]
    fn deadline_kills_waits_and_reaps_slow_worker_without_orphan() {
        let temporary = tempfile::tempdir().unwrap();
        let pid_file = temporary.path().join("worker.pid");
        let orphan_marker = temporary.path().join("worker-survived");
        let launcher = TestWorkerLauncher::new(fixture_path(), "slow")
            .with_arguments([pid_file.clone(), orphan_marker.clone()]);
        let started = Instant::now();
        let error = capture_bounded(
            spawn_restricted(&launcher).unwrap(),
            SupervisorIoLimits {
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            },
            Duration::from_secs(1),
        )
        .unwrap_err();

        assert!(
            matches!(error, crate::DiscoveryError::LimitExceeded(message) if message.contains("wall-clock"))
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!orphan_marker.exists());
        let pid = std::fs::read_to_string(pid_file).unwrap();
        #[cfg(target_os = "linux")]
        assert!(
            !Path::new("/proc").join(pid.trim()).exists(),
            "timed-out worker must be killed and reaped"
        );
    }

    #[test]
    fn zero_deadline_is_rejected_before_waiting() {
        let launcher = TestWorkerLauncher::new(fixture_path(), "slow");
        let error = capture_bounded(
            spawn_restricted(&launcher).unwrap(),
            SupervisorIoLimits {
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            },
            Duration::ZERO,
        )
        .unwrap_err();

        assert!(
            matches!(error, crate::DiscoveryError::InvalidInput(message) if message.contains("deadline"))
        );
    }

    #[test]
    fn successful_worker_preserves_provenance_and_reports_process_isolation() {
        let response = run_fixture("valid").unwrap();

        assert_eq!(response.provenance, fixture_provenance());
        assert_eq!(
            response.execution.isolation,
            super::super::ProbeIsolation::Process
        );
        assert_eq!(
            response.execution.output,
            serde_json::json!({"fixture": true})
        );
    }

    #[test]
    fn crashing_worker_maps_to_typed_crash_without_protocol_decode() {
        let error = run_fixture("crash").unwrap_err();
        assert!(matches!(
            error,
            crate::DiscoveryError::ProbeWorkerCrashed { .. }
        ));
    }

    #[test]
    fn nonzero_worker_maps_exit_code_without_leaking_stderr() {
        let error = run_fixture("nonzero").unwrap_err();
        assert!(matches!(
            error,
            crate::DiscoveryError::ProbeWorkerExited { code: 17 }
        ));
        assert!(!error.to_string().contains("SECRET_DIAGNOSTIC"));
    }

    #[test]
    fn malformed_success_result_maps_to_typed_protocol_failure() {
        let error = run_fixture("malformed").unwrap_err();
        assert!(matches!(
            error,
            crate::DiscoveryError::ProbeWorkerProtocol(_)
        ));
    }

    #[test]
    fn changed_worker_provenance_is_rejected() {
        let error = run_fixture("wrong-provenance").unwrap_err();
        assert!(
            matches!(error, crate::DiscoveryError::ProbeWorkerProtocol(message) if message.contains("provenance"))
        );
    }
}
