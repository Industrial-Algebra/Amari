//! Safe-tensor checkpoint contracts (0.25 Cohort 8, Task 43).
//!
//! One test per enumerated rule of the task:
//!
//! - **score round trip** — a saved and loaded ranker scores bit-identically,
//!   with a bit-identical optimizer state;
//! - **schema/model metadata** — the artifact carries the frozen feature
//!   schema digest and version, the `ia-mlp` spec and its canonical hashes,
//!   the checkpoint schema version, and the training step;
//! - **wrong shape/schema rejection** — a same-span but differently shaped
//!   tensor is rejected by the seam's layout check, and a different
//!   architecture is rejected against the caller's expected configuration;
//! - **truncated/oversized/nonfinite rejection** — through the Katakuri
//!   trust-policy seam; nothing here re-implements safe opening or scanning;
//! - **symlink rejection** — `regular_file_only` refuses a symlinked
//!   artifact;
//! - **canonical parameter ordering** — weights in `expected_tensors` order,
//!   moments in S6's lexicographic order, artifacts byte-stable;
//! - **optimizer all-or-nothing** — a file missing one moment is rejected;
//! - **resume-training determinism** — `train N, save, load, train M` equals
//!   `train N+M`, artifacts included;
//! - **path sanitization** — no rejection quotes a path or the file's text.
//!
//! Fixtures are frozen [`FeatureVector`]s: the encode path is Task 40's
//! surface and is exercised there, not here.

#![cfg(feature = "neural")]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use sha2::{Digest, Sha256};

use amari_rewrite::neural::{
    checkpoint_policy, train_from_seed, train_with_state, CheckpointError, ExampleOrder,
    FeatureSchemaV1, FeatureVector, MlpRanker, OptimizerRejection, RankerCheckpoint, RankerConfig,
    TrainConfig, TrainingExample, TrainingOutcome, WeightsRejection, ADAMW_MOMENT_SUFFIX,
    ADAMW_VELOCITY_SUFFIX, CANDIDATE_FEATURE_WIDTH, MAX_CHECKPOINT_STEPS,
    MAX_OPTIMIZER_CHECKPOINT_BYTES, MAX_WEIGHTS_CHECKPOINT_BYTES, META_CHECKPOINT_VERSION,
    META_FEATURE_SCHEMA_HASH, META_FEATURE_SCHEMA_VERSION, META_RANKER_SPEC_HASH,
    META_TRAINING_STEP, META_WEIGHTS_HASH,
};
use katakuri::checkpoint::TrustPolicy;

// ── fixtures ────────────────────────────────────────────────────────

/// A ranker specification, parsed through the same JSON seam callers use.
fn config(hidden: &[u32]) -> RankerConfig {
    let hidden = hidden
        .iter()
        .map(|width| format!("{{\"width\":{width}}}"))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":{},\
         \"hidden\":[{hidden}],\"head\":{{\"width\":1}},\
         \"hidden_activation\":\"relu\"}}",
        CANDIDATE_FEATURE_WIDTH
    );
    RankerConfig::from_spec_json(&json).expect("valid ranker spec")
}

fn features(pairs: &[(usize, f32)]) -> FeatureVector {
    let mut values = [0.0f32; CANDIDATE_FEATURE_WIDTH];
    for (index, value) in pairs {
        values[*index] = *value;
    }
    FeatureVector::new(values).expect("valid feature lanes")
}

fn example(chosen: &[(usize, f32)], unchosen: &[(usize, f32)]) -> TrainingExample {
    TrainingExample::new(features(chosen), features(unchosen))
}

/// A separable corpus that converges.
fn separable_corpus() -> Vec<TrainingExample> {
    vec![
        example(&[(0, 1.0)], &[]),
        example(&[(0, 0.75), (1, 0.25)], &[(0, 0.25), (1, 0.25)]),
        example(&[(0, 0.5), (2, 0.5)], &[(0, 0.0), (2, 0.5)]),
    ]
}

/// A corpus that can never converge: the two examples ask for opposite
/// orderings of the same two points, so one hinge is always at full margin
/// while the other drives a non-zero gradient. Every step therefore applies
/// an AdamW update, which makes the resume bookkeeping exact.
fn contradictory_corpus() -> Vec<TrainingExample> {
    vec![example(&[(0, 1.0)], &[]), example(&[], &[(0, 1.0)])]
}

fn train_config(max_steps: usize) -> TrainConfig {
    TrainConfig::new(
        0.05,
        0.9,
        0.999,
        1e-8,
        0.0,
        1.0,
        max_steps,
        0x5eed,
        ExampleOrder::Canonical,
    )
    .expect("valid training config")
}

fn run(config: &RankerConfig, steps: usize, corpus: &[TrainingExample]) -> TrainingOutcome {
    train_from_seed(config, &train_config(steps), corpus).expect("deterministic training run")
}

fn checkpoint_of(config: &RankerConfig, outcome: &TrainingOutcome) -> RankerCheckpoint {
    RankerCheckpoint::new(
        config.clone(),
        outcome.parameters().clone(),
        outcome.optimizer_state().clone(),
    )
    .expect("consistent checkpoint")
}

/// Every optimizer entry's bits: bit equality rather than `f32` equality.
fn entry_bits(entries: &[amari_rewrite::neural::OptimizerStateEntry]) -> Vec<(String, Vec<u32>)> {
    entries
        .iter()
        .map(|entry| {
            (
                entry.name().to_string(),
                entry.values().iter().map(|value| value.to_bits()).collect(),
            )
        })
        .collect()
}

/// A self-cleaning temporary directory (no `tempfile` dependency).
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "amari-task43-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn weights(&self) -> PathBuf {
        self.path.join("ranker.safetensors")
    }

    fn optimizer(&self) -> PathBuf {
        self.path.join("ranker.adamw.safetensors")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn save(dir: &TempDir, checkpoint: &RankerCheckpoint) {
    checkpoint
        .save(&dir.weights(), &dir.optimizer())
        .expect("checkpoint save");
}

fn load(
    dir: &TempDir,
    expected: &RankerConfig,
    resume: &TrainConfig,
) -> Result<RankerCheckpoint, CheckpointError> {
    RankerCheckpoint::load(
        &dir.weights(),
        &dir.optimizer(),
        &checkpoint_policy(),
        expected,
        resume,
    )
}

/// Save the standard fixture, then load it.
fn saved_fixture(dir: &TempDir, config: &RankerConfig) -> (TrainingOutcome, TrainConfig) {
    let corpus = separable_corpus();
    let settings = train_config(16);
    let outcome = train_from_seed(config, &settings, &corpus).expect("training run");
    save(dir, &checkpoint_of(config, &outcome));
    (outcome, settings)
}

// ── artifact inspection (raw bytes; no library round trip) ──────────

fn split_artifact(bytes: &[u8]) -> (String, Vec<u8>) {
    let length = u64::from_le_bytes(bytes[..8].try_into().expect("8-byte prefix")) as usize;
    let header = String::from_utf8(bytes[8..8 + length].to_vec()).expect("utf-8 header");
    (header, bytes[8 + length..].to_vec())
}

fn header_of(path: &Path) -> String {
    split_artifact(&std::fs::read(path).expect("read artifact")).0
}

fn header_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&header_of(path)).expect("header is JSON")
}

fn metadata_json(path: &Path) -> serde_json::Value {
    header_json(path)["__metadata__"].clone()
}

fn data_section(path: &Path) -> Vec<u8> {
    split_artifact(&std::fs::read(path).expect("read artifact")).1
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The physical order of the named entries in the raw header text.
fn physical_order(header: &str, names: &[String]) -> Vec<String> {
    let mut positioned: Vec<(usize, String)> = names
        .iter()
        .map(|name| {
            let needle = format!("\"{name}\":");
            let position = header
                .find(&needle)
                .unwrap_or_else(|| panic!("entry for {name} present"));
            (position, name.clone())
        })
        .collect();
    positioned.sort_by_key(|(position, _)| *position);
    positioned.into_iter().map(|(_, name)| name).collect()
}

fn rewrite_artifact(path: &Path, header: &str, data: &[u8]) {
    let mut bytes = Vec::with_capacity(8 + header.len() + data.len());
    bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(data);
    std::fs::write(path, bytes).expect("rewrite artifact");
}

/// The end (exclusive) of the JSON string literal starting at `start - 1`.
///
/// Scans for an unescaped quote: metadata values include the `ia.spec`
/// document, whose quotes arrive escaped (`\"`), so a plain `find('"')` would
/// cut the value in half.
fn string_end(header: &str, start: usize) -> usize {
    let bytes = header.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index,
            _ => index += 1,
        }
    }
    panic!("unterminated metadata value");
}

/// Replace one `__metadata__` value, rebuilding the prefix so a length change
/// is legal.
fn set_metadata(path: &Path, key: &str, value: &str) {
    let (header, data) = split_artifact(&std::fs::read(path).expect("read artifact"));
    let needle = format!("\"{key}\":\"");
    let start = header.find(&needle).expect("metadata key present") + needle.len();
    let end = string_end(&header, start);
    let edited = format!("{}{}{}", &header[..start], value, &header[end..]);
    rewrite_artifact(path, &edited, &data);
}

/// Delete one `__metadata__` entry.
fn remove_metadata(path: &Path, key: &str) {
    let (header, data) = split_artifact(&std::fs::read(path).expect("read artifact"));
    let needle = format!("\"{key}\":\"");
    let key_start = header.find(&needle).expect("metadata key present");
    let value_end = string_end(&header, key_start + needle.len()) + 1;
    let mut start = key_start;
    if start > 0 && header.as_bytes()[start - 1] == b',' {
        start -= 1;
    }
    let edited = format!("{}{}", &header[..start], &header[value_end..]);
    rewrite_artifact(path, &edited, &data);
}

/// Delete one tensor entry from the header; its bytes stay behind as an
/// unreferenced trailing region, which safetensors permits.
fn remove_tensor_entry(path: &Path, name: &str) {
    let (header, data) = split_artifact(&std::fs::read(path).expect("read artifact"));
    let needle = format!("\"{name}\":{{");
    let start = header.find(&needle).expect("tensor entry present");
    let relative_end = header[start + needle.len()..]
        .find('}')
        .expect("entry object closes");
    let end = start + needle.len() + relative_end + 1;
    let mut cut = start;
    let mut tail = end;
    if tail < header.len() && header.as_bytes()[tail] == b',' {
        tail += 1;
    } else if cut > 0 && header.as_bytes()[cut - 1] == b',' {
        cut -= 1;
    }
    let edited = format!("{}{}", &header[..cut], &header[tail..]);
    rewrite_artifact(path, &edited, &data);
}

/// Replace one tensor's declared shape (the span is unchanged, so the file
/// stays parse-valid while the shape diverges from what the bytes say).
fn reshape_entry(path: &Path, name: &str, from: &str, to: &str) {
    let (header, data) = split_artifact(&std::fs::read(path).expect("read artifact"));
    let needle = format!("\"{name}\":{{\"dtype\":\"F32\",\"shape\":{from}");
    let position = header
        .find(&needle)
        .unwrap_or_else(|| panic!("entry shape {from} for {name} present"));
    let edited = format!(
        "{}{}",
        &header[..position],
        header[position..].replacen(from, to, 1)
    );
    rewrite_artifact(path, &edited, &data);
}

/// Overwrite the first tensor element with NaN and repair `ia.model-hash`, so
/// the file reaches the policy's value scan rather than failing at the hash
/// check (the seam checks hashes first, by design).
fn corrupt_first_tensor_to_nan(path: &Path) {
    let mut bytes = std::fs::read(path).expect("read artifact");
    let length = u64::from_le_bytes(bytes[..8].try_into().expect("8-byte prefix")) as usize;
    let header: serde_json::Value =
        serde_json::from_slice(&bytes[8..8 + length]).expect("header JSON");
    let start = header
        .as_object()
        .expect("header object")
        .iter()
        .filter(|(name, _)| name.as_str() != "__metadata__")
        .map(|(_, entry)| entry["data_offsets"][0].as_u64().expect("offset") as usize)
        .min()
        .expect("at least one tensor");
    let offset = 8 + length + start;
    bytes[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let hash = hex_sha256(&bytes[8 + length..]);
    std::fs::write(path, bytes).expect("write corrupted artifact");
    set_metadata(path, "ia.model-hash", &hash);
}

// ── round trip ──────────────────────────────────────────────────────

#[test]
fn save_then_load_round_trips_weights_optimizer_state_and_scores() {
    let config = config(&[3]);
    let corpus = separable_corpus();
    let settings = train_config(64);
    let outcome = train_from_seed(&config, &settings, &corpus).expect("training run");
    let checkpoint = checkpoint_of(&config, &outcome);
    let dir = TempDir::new("roundtrip");
    save(&dir, &checkpoint);

    let loaded = load(&dir, &config, &settings).expect("valid checkpoint");
    assert_eq!(
        loaded.training_step(),
        outcome.optimizer_state().step() as u64,
        "the checkpoint step is the optimizer's S6 step counter"
    );
    assert_eq!(
        loaded.weights().digest(),
        outcome.parameters().digest(),
        "the loaded weights must be bit-identical (the digest covers every element's bits)"
    );
    assert_eq!(
        entry_bits(&loaded.optimizer_state().optimizer_state_entries()),
        entry_bits(&outcome.optimizer_state().optimizer_state_entries()),
        "the loaded optimizer state must be bit-identical"
    );
    assert_eq!(
        loaded.optimizer_state().step(),
        outcome.optimizer_state().step()
    );
    assert_eq!(loaded.config(), &config);

    // Constructing the model is a separate, post-validation step.
    let before = MlpRanker::new(config.clone(), outcome.parameters().clone()).expect("ranker");
    let after = loaded.ranker().expect("validated weights bind to the spec");
    for probe in [
        features(&[]),
        features(&[(0, 1.0)]),
        features(&[(0, 0.5), (7, 0.25), (63, 1.0)]),
        features(&[(1, 0.125), (2, 0.875)]),
    ] {
        assert_eq!(
            after.score_features(&probe).expect("score").to_bits(),
            before.score_features(&probe).expect("score").to_bits(),
            "a loaded ranker must score bit-identically"
        );
    }
}

#[test]
fn artifacts_carry_the_binding_metadata() {
    let config = config(&[3]);
    let dir = TempDir::new("metadata");
    let (outcome, _) = saved_fixture(&dir, &config);
    let step = outcome.optimizer_state().step() as u64;
    let spec_json = config.spec().canonical_json();

    let metadata = metadata_json(&dir.weights());
    assert_eq!(
        metadata["ia.spec"].as_str(),
        Some(spec_json.as_str()),
        "the architecture is embedded as canonical `ia-mlp` JSON"
    );
    assert_eq!(
        metadata["ia.schema-hash"].as_str(),
        Some(hex_sha256(spec_json.as_bytes()).as_str()),
        "`ia.schema-hash` is the canonical spec digest (Katakuri's convention)"
    );
    assert_eq!(
        metadata["ia.model-hash"].as_str(),
        Some(hex_sha256(&data_section(&dir.weights())).as_str()),
        "`ia.model-hash` is the data-section digest the seam verifies"
    );
    assert_eq!(
        metadata[META_CHECKPOINT_VERSION].as_str(),
        Some("1"),
        "the checkpoint schema version is pinned"
    );
    assert_eq!(
        metadata[META_FEATURE_SCHEMA_VERSION].as_str(),
        Some(FeatureSchemaV1::VERSION.to_string().as_str()),
        "the feature schema version is recorded"
    );
    assert_eq!(
        metadata[META_FEATURE_SCHEMA_HASH].as_str(),
        Some(FeatureSchemaV1.digest().to_hex().as_str()),
        "the frozen feature schema digest binds features to weights"
    );
    assert_eq!(
        metadata[META_RANKER_SPEC_HASH].as_str(),
        Some(config.digest().to_hex().as_str()),
        "the Amari ranker-spec digest is recorded alongside the Katakuri spec"
    );
    assert_eq!(
        metadata[META_WEIGHTS_HASH].as_str(),
        Some(outcome.parameters().digest().to_hex().as_str()),
        "the canonical weight digest is recorded"
    );
    assert_eq!(
        metadata[META_TRAINING_STEP].as_str(),
        Some(step.to_string().as_str()),
        "the training step is recorded"
    );

    let optimizer = metadata_json(&dir.optimizer());
    assert_eq!(optimizer["optimizer_state_version"].as_str(), Some("1"));
    assert_eq!(optimizer["optimizer"].as_str(), Some("adamw"));
    assert_eq!(
        optimizer["step"].as_str(),
        Some(step.to_string().as_str()),
        "S6 carries the step counter as metadata"
    );
    assert_eq!(
        optimizer["ia.spec"].as_str(),
        Some(spec_json.as_str()),
        "the optimizer artifact binds to the same architecture"
    );
    assert_eq!(
        optimizer["ia.model-hash"].as_str(),
        Some(hex_sha256(&data_section(&dir.optimizer())).as_str())
    );
}

#[test]
fn artifacts_are_byte_stable_and_canonically_ordered() {
    let config = config(&[3, 2]);
    let first = TempDir::new("stable-a");
    let second = TempDir::new("stable-b");
    let (outcome, settings) = saved_fixture(&first, &config);
    let checkpoint = checkpoint_of(&config, &outcome);
    save(&second, &checkpoint);

    for paths in [
        (first.weights(), second.weights()),
        (first.optimizer(), second.optimizer()),
    ] {
        assert_eq!(
            std::fs::read(&paths.0).expect("read"),
            std::fs::read(&paths.1).expect("read"),
            "the same inputs must produce the same bytes"
        );
    }

    // Re-saving a loaded checkpoint reproduces its artifacts exactly.
    let third = TempDir::new("stable-c");
    let loaded = load(&first, &config, &settings).expect("valid checkpoint");
    save(&third, &loaded);
    assert_eq!(
        std::fs::read(first.weights()).expect("read"),
        std::fs::read(third.weights()).expect("read"),
        "load then save must be byte-identical to the original artifact"
    );
    assert_eq!(
        std::fs::read(first.optimizer()).expect("read"),
        std::fs::read(third.optimizer()).expect("read")
    );

    // Weights: Katakuri's canonical parameter order.
    let parameter_names: Vec<String> = config
        .expected_tensors()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        physical_order(&header_of(&first.weights()), &parameter_names),
        parameter_names,
        "weight tensors are written in `expected_tensors` order"
    );

    // Moments: S6's lexicographic canonical order.
    let mut moment_names = Vec::new();
    for (name, _) in config.expected_tensors() {
        moment_names.push(format!("{name}{ADAMW_MOMENT_SUFFIX}"));
        moment_names.push(format!("{name}{ADAMW_VELOCITY_SUFFIX}"));
    }
    moment_names.sort();
    assert_eq!(
        physical_order(&header_of(&first.optimizer()), &moment_names),
        moment_names,
        "moment tensors are written in S6's lexicographic canonical order"
    );

    // Admission ceilings are per artifact: moments are twice the weights,
    // and both leave room for a header on top of the largest legal data
    // section.
    const LARGEST_DATA_SECTION: u64 = 16_777_216 * 4;
    assert!(MAX_WEIGHTS_CHECKPOINT_BYTES > LARGEST_DATA_SECTION);
    assert!(MAX_OPTIMIZER_CHECKPOINT_BYTES > 2 * LARGEST_DATA_SECTION);
    assert!(MAX_CHECKPOINT_STEPS >= 65_536);
}

// ── rejection ───────────────────────────────────────────────────────

#[test]
fn load_rejects_a_mismatched_architecture() {
    let dir = TempDir::new("wrong-arch");
    saved_fixture(&dir, &config(&[3]));
    match load(&dir, &config(&[4]), &train_config(16)) {
        Err(CheckpointError::ArchitectureMismatch) => {}
        other => panic!("expected a typed architecture mismatch, got {other:?}"),
    }
    // The same widths in a different layer order is also a different model.
    match load(&dir, &config(&[2, 1]), &train_config(16)) {
        Err(CheckpointError::ArchitectureMismatch) => {}
        other => panic!("expected a typed architecture mismatch, got {other:?}"),
    }
}

#[test]
fn load_rejects_a_tensor_whose_shape_disagrees_with_the_spec() {
    let dir = TempDir::new("wrong-shape");
    saved_fixture(&dir, &config(&[3]));
    // 3 x 64 f32 = 192 f32 = the same 768-byte span, so the file stays
    // parse-valid and the divergence is a genuine layout disagreement.
    reshape_entry(&dir.weights(), "hidden.0.weight", "[3,64]", "[192]");
    match load(&dir, &config(&[3]), &train_config(16)) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::Layout,
        }) => {}
        other => panic!("expected a typed layout rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_an_oversized_artifact() {
    let dir = TempDir::new("oversized");
    let (_, settings) = saved_fixture(&dir, &config(&[3]));
    let policy = TrustPolicy {
        byte_cap: 64,
        ..checkpoint_policy()
    };
    match RankerCheckpoint::load(
        &dir.weights(),
        &dir.optimizer(),
        &policy,
        &config(&[3]),
        &settings,
    ) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::TooLarge,
        }) => {}
        other => panic!("expected a typed size rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_a_truncated_artifact() {
    let dir = TempDir::new("truncated");
    let (_, settings) = saved_fixture(&dir, &config(&[3]));
    for path in [dir.weights(), dir.optimizer()] {
        let bytes = std::fs::read(&path).expect("read");
        std::fs::write(&path, &bytes[..bytes.len() - 4]).expect("truncate");
    }
    match RankerCheckpoint::load(
        &dir.weights(),
        &dir.optimizer(),
        &checkpoint_policy(),
        &config(&[3]),
        &settings,
    ) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::Parse,
        }) => {}
        other => panic!("expected a typed parse rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_nonfinite_values() {
    let config = config(&[3]);
    let dir = TempDir::new("nonfinite");
    let (_, settings) = saved_fixture(&dir, &config);
    corrupt_first_tensor_to_nan(&dir.weights());
    match load(&dir, &config, &settings) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::Nonfinite,
        }) => {}
        other => panic!("expected a typed nonfinite rejection, got {other:?}"),
    }

    let dir = TempDir::new("nonfinite-moments");
    let (_, settings) = saved_fixture(&dir, &config);
    corrupt_first_tensor_to_nan(&dir.optimizer());
    match load(&dir, &config, &settings) {
        Err(CheckpointError::OptimizerRejected {
            kind: OptimizerRejection::Nonfinite,
        }) => {}
        other => panic!("expected a typed nonfinite rejection, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn load_rejects_a_symlinked_artifact() {
    use std::os::unix::fs::symlink;
    let config = config(&[3]);
    let dir = TempDir::new("symlink");
    let (_, settings) = saved_fixture(&dir, &config);

    let weights_link = dir.path.join("weights-link.safetensors");
    symlink(dir.weights(), &weights_link).expect("symlink");
    match RankerCheckpoint::load(
        &weights_link,
        &dir.optimizer(),
        &checkpoint_policy(),
        &config,
        &settings,
    ) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::NotRegularFile,
        }) => {}
        other => panic!("expected a typed symlink rejection, got {other:?}"),
    }

    let optimizer_link = dir.path.join("optimizer-link.safetensors");
    symlink(dir.optimizer(), &optimizer_link).expect("symlink");
    match RankerCheckpoint::load(
        &dir.weights(),
        &optimizer_link,
        &checkpoint_policy(),
        &config,
        &settings,
    ) {
        Err(CheckpointError::OptimizerRejected {
            kind: OptimizerRejection::NotRegularFile,
        }) => {}
        other => panic!("expected a typed symlink rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_a_different_feature_schema() {
    let config = config(&[3]);
    let dir = TempDir::new("schema-drift");
    let (_, settings) = saved_fixture(&dir, &config);
    set_metadata(&dir.weights(), META_FEATURE_SCHEMA_HASH, &"0".repeat(64));
    match load(&dir, &config, &settings) {
        Err(CheckpointError::FeatureSchemaDrift) => {}
        other => panic!("expected a feature-schema drift rejection, got {other:?}"),
    }

    let dir = TempDir::new("schema-version-drift");
    let (_, settings) = saved_fixture(&dir, &config);
    set_metadata(&dir.weights(), META_FEATURE_SCHEMA_VERSION, "2");
    match load(&dir, &config, &settings) {
        Err(CheckpointError::FeatureSchemaDrift) => {}
        other => panic!("expected a feature-schema drift rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_an_unknown_checkpoint_version() {
    let config = config(&[3]);
    let dir = TempDir::new("checkpoint-version");
    let (_, settings) = saved_fixture(&dir, &config);
    set_metadata(&dir.weights(), META_CHECKPOINT_VERSION, "2");
    match load(&dir, &config, &settings) {
        Err(CheckpointError::UnsupportedCheckpointVersion { found: 2 }) => {}
        other => panic!("expected a typed version rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_missing_metadata() {
    let config = config(&[3]);
    let dir = TempDir::new("missing-metadata");
    let (_, settings) = saved_fixture(&dir, &config);
    remove_metadata(&dir.weights(), META_TRAINING_STEP);
    match load(&dir, &config, &settings) {
        Err(CheckpointError::InvalidMetadata {
            key: META_TRAINING_STEP,
        }) => {}
        other => panic!("expected a typed metadata rejection, got {other:?}"),
    }

    // `ia.spec` is Katakuri's requirement, enforced by the seam itself.
    let dir = TempDir::new("missing-spec");
    let (_, settings) = saved_fixture(&dir, &config);
    remove_metadata(&dir.weights(), "ia.spec");
    match load(&dir, &config, &settings) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::Metadata,
        }) => {}
        other => panic!("expected a typed metadata rejection, got {other:?}"),
    }
}

#[test]
fn optimizer_state_is_all_or_nothing() {
    let config = config(&[3]);
    let dir = TempDir::new("all-or-nothing");
    let (_, settings) = saved_fixture(&dir, &config);
    remove_tensor_entry(&dir.optimizer(), "hidden.0.weight.adamw.v");
    match load(&dir, &config, &settings) {
        Err(CheckpointError::OptimizerRejected {
            kind: OptimizerRejection::Moments,
        }) => {}
        other => panic!("expected an all-or-nothing rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_an_unbounded_step_counter() {
    let config = config(&[3]);
    let dir = TempDir::new("step-ceiling");
    let (_, settings) = saved_fixture(&dir, &config);
    set_metadata(&dir.optimizer(), "step", &u64::MAX.to_string());
    match load(&dir, &config, &settings) {
        Err(CheckpointError::LimitExceeded {
            resource: "checkpoint steps",
            ..
        }) => {}
        other => panic!("expected a typed step-ceiling rejection, got {other:?}"),
    }
}

#[test]
fn load_rejects_artifacts_from_different_runs() {
    let config = config(&[3]);
    let corpus = contradictory_corpus();
    let first = TempDir::new("pair-a");
    let second = TempDir::new("pair-b");
    // Two different runs of the same architecture: never converging, so the
    // step counters are exactly the budgets.
    let head = run(&config, 8, &corpus);
    let tail = run(&config, 12, &corpus);
    save(&first, &checkpoint_of(&config, &head));
    save(&second, &checkpoint_of(&config, &tail));
    let settings = train_config(4);

    match RankerCheckpoint::load(
        &first.weights(),
        &second.optimizer(),
        &checkpoint_policy(),
        &config,
        &settings,
    ) {
        Err(CheckpointError::InconsistentCheckpoint { .. }) => {}
        other => panic!("expected a typed pair inconsistency, got {other:?}"),
    }

    // The weight artifact's own step claim is checked too.
    let step = metadata_json(&first.weights())[META_TRAINING_STEP]
        .as_str()
        .and_then(|value| value.parse::<u64>().ok())
        .expect("recorded step");
    set_metadata(
        &first.weights(),
        META_TRAINING_STEP,
        &(step + 1).to_string(),
    );
    match RankerCheckpoint::load(
        &first.weights(),
        &first.optimizer(),
        &checkpoint_policy(),
        &config,
        &settings,
    ) {
        Err(CheckpointError::InconsistentCheckpoint { .. }) => {}
        other => panic!("expected a typed pair inconsistency, got {other:?}"),
    }
}

// ── resume ──────────────────────────────────────────────────────────

#[test]
fn resume_training_matches_uninterrupted_training() {
    let config = config(&[3]);
    let corpus = contradictory_corpus();
    let total = 10usize;
    let first = 4usize;
    let rest = total - first;
    let uninterrupted = run(&config, total, &corpus);
    assert!(
        !uninterrupted.is_complete(),
        "the contradictory corpus must not converge, so both runs are budget-truncated"
    );
    assert_eq!(uninterrupted.steps_run(), total);
    assert_eq!(
        uninterrupted.optimizer_state().step(),
        total,
        "every step of a contradictory corpus applies an update"
    );

    let head = run(&config, first, &corpus);
    assert_eq!(head.steps_run(), first);
    let dir = TempDir::new("resume");
    save(&dir, &checkpoint_of(&config, &head));

    // The resumed leg restarts the example order, so `first` must be a whole
    // number of epochs (2 examples per epoch here); the state at that point is
    // then exactly the start-of-epoch state the uninterrupted run had.
    let resumed_checkpoint = load(&dir, &config, &train_config(rest)).expect("valid checkpoint");
    let resumed = train_with_state(
        &config,
        resumed_checkpoint.weights(),
        resumed_checkpoint.optimizer_state().clone(),
        &train_config(rest),
        &corpus,
    )
    .expect("resumed run");

    assert_eq!(
        resumed.steps_run(),
        rest,
        "the resumed leg ran its own budget"
    );
    assert_eq!(
        resumed.parameters().digest(),
        uninterrupted.parameters().digest(),
        "resumed training must reproduce the uninterrupted parameters bit for bit"
    );
    assert_eq!(
        entry_bits(&resumed.optimizer_state().optimizer_state_entries()),
        entry_bits(&uninterrupted.optimizer_state().optimizer_state_entries()),
        "resumed training must reproduce the optimizer state bit for bit"
    );
    assert_eq!(
        resumed.optimizer_state().step(),
        uninterrupted.optimizer_state().step(),
        "the resumed state continues the S6 step counter"
    );

    // The artifacts are byte-identical, which is the checkpoint's own claim.
    let resumed_dir = TempDir::new("resume-artifact");
    let uninterrupted_dir = TempDir::new("uninterrupted-artifact");
    save(&resumed_dir, &checkpoint_of(&config, &resumed));
    save(&uninterrupted_dir, &checkpoint_of(&config, &uninterrupted));
    assert_eq!(
        std::fs::read(resumed_dir.weights()).expect("read"),
        std::fs::read(uninterrupted_dir.weights()).expect("read"),
        "the resumed run's weights artifact is byte-identical"
    );
    assert_eq!(
        std::fs::read(resumed_dir.optimizer()).expect("read"),
        std::fs::read(uninterrupted_dir.optimizer()).expect("read"),
        "the resumed run's optimizer artifact is byte-identical"
    );
}

// ── admission and diagnostics ───────────────────────────────────────

#[test]
fn policy_must_be_strict() {
    let config = config(&[3]);
    let dir = TempDir::new("policy");
    let (_, settings) = saved_fixture(&dir, &config);

    for (flag, lax) in [
        ("regular_file_only", {
            let mut policy = checkpoint_policy();
            policy.regular_file_only = false;
            policy
        }),
        ("verify_schema_hash", {
            let mut policy = checkpoint_policy();
            policy.verify_schema_hash = false;
            policy
        }),
        ("verify_model_hash", {
            let mut policy = checkpoint_policy();
            policy.verify_model_hash = false;
            policy
        }),
        ("reject_nonfinite", {
            let mut policy = checkpoint_policy();
            policy.reject_nonfinite = false;
            policy
        }),
    ] {
        match RankerCheckpoint::load(&dir.weights(), &dir.optimizer(), &lax, &config, &settings) {
            Err(CheckpointError::UnsafePolicy { flag: reported }) => assert_eq!(reported, flag),
            other => panic!("expected an unsafe-policy rejection for {flag}, got {other:?}"),
        }
    }
}

#[test]
fn rejections_are_typed_bounded_and_path_free() {
    let config = config(&[3]);
    let dir = TempDir::new("path-marker-do-not-leak");
    let marker = dir
        .path
        .file_name()
        .expect("temp dir name")
        .to_string_lossy()
        .to_string();
    let settings = train_config(16);

    // A missing artifact (I/O failure, not a parse failure).
    let missing = load(&dir, &config, &settings).expect_err("no files written");
    assert!(
        matches!(
            missing,
            CheckpointError::WeightsRejected {
                kind: WeightsRejection::Io
            }
        ),
        "a missing file is a typed I/O rejection, got {missing:?}"
    );
    for rendered in [missing.to_string(), format!("{missing:?}")] {
        assert!(
            !rendered.contains(&marker),
            "a rejection must not leak the path: {rendered}"
        );
        assert!(
            rendered.len() < 256,
            "a rejection must stay bounded: {rendered}"
        );
    }

    // A malformed artifact: the file's own text must never be quoted back.
    let dir = TempDir::new("path-marker-malformed");
    let (_, settings) = saved_fixture(&dir, &config);
    let bytes = std::fs::read(dir.weights()).expect("read");
    std::fs::write(dir.weights(), &bytes[..bytes.len() / 2]).expect("truncate");
    let malformed = load(&dir, &config, &settings).expect_err("truncated artifact");
    let rendered = malformed.to_string();
    assert!(!rendered.contains(&marker));
    assert!(
        rendered.len() < 256,
        "a rejection must stay bounded: {rendered}"
    );
}

#[test]
fn checkpoint_construction_rejects_mismatched_components() {
    let narrow = config(&[3]);
    let wide = config(&[4]);
    let corpus = separable_corpus();
    let outcome = train_from_seed(&narrow, &train_config(16), &corpus).expect("training run");

    match RankerCheckpoint::new(
        wide.clone(),
        outcome.parameters().clone(),
        outcome.optimizer_state().clone(),
    ) {
        Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::Layout,
        }) => {}
        other => panic!("expected a typed layout rejection, got {other:?}"),
    }

    let wide_outcome = train_from_seed(&wide, &train_config(16), &corpus).expect("training run");
    match RankerCheckpoint::new(
        wide.clone(),
        wide_outcome.parameters().clone(),
        outcome.optimizer_state().clone(),
    ) {
        Err(CheckpointError::OptimizerRejected {
            kind: OptimizerRejection::Moments,
        }) => {}
        other => panic!("expected a typed moment rejection, got {other:?}"),
    }
}
