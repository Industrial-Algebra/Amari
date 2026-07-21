// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public cooperative probe-engine contracts.

#[cfg(feature = "standard-probes")]
use amari_discovery::ProbeIsolation;
use amari_discovery::{DiscoveryError, ProbeEngine, TropicalViterbiRequest};
use serde_json::json;

const CGT_NIM_SUM: &str = "amari-probe:cgt:nim-sum:v1";
const CORE_PRODUCT: &str = "amari-probe:core:geometric-product:v1";
const POLYNOMIAL_DERIVATIVE: &str = "amari-probe:dual:polynomial-derivative:v1";
const SHORTEST_PATH: &str = "amari-probe:network:shortest-path:v1";
const PARETO_FRONT: &str = "amari-probe:optimization:pareto-front:v1";
const REWRITE_NORMALIZE: &str = "amari-probe:rewrite:normalize:v1";
const RECALL: &str = "amari-probe:holographic:recall:v1";
const SUPERPOSITION: &str = "amari-probe:holographic:superposition:v1";
const RATIONAL_ARITHMETIC: &str = "amari-probe:surreal:rational-arithmetic:v1";
const SURCOMPLEX_DIVISION: &str = "amari-probe:surcomplex:rational-division:v1";
const VITERBI: &str = "amari-probe:tropical:viterbi:v1";
const KNOWN_UNREGISTERED: &str = "amari-probe:rewrite:predecessors:v1";

fn request() -> TropicalViterbiRequest {
    TropicalViterbiRequest {
        transitions: vec![vec![-1.0, -2.0], vec![-2.0, -1.0]],
        emissions: vec![vec![-1.0, -3.0], vec![-3.0, -1.0]],
        observations: vec![0, 1, 0],
    }
}

#[test]
fn engine_derives_executable_state_from_the_private_registry() {
    let engine = ProbeEngine::new().unwrap();
    let cgt = CGT_NIM_SUM.parse().unwrap();
    let core = CORE_PRODUCT.parse().unwrap();
    let dual = POLYNOMIAL_DERIVATIVE.parse().unwrap();
    let network = SHORTEST_PATH.parse().unwrap();
    let optimization = PARETO_FRONT.parse().unwrap();
    let rewrite_normalize = REWRITE_NORMALIZE.parse().unwrap();
    let recall = RECALL.parse().unwrap();
    let superposition = SUPERPOSITION.parse().unwrap();
    let surreal = RATIONAL_ARITHMETIC.parse().unwrap();
    let surcomplex = SURCOMPLEX_DIVISION.parse().unwrap();
    let viterbi = VITERBI.parse().unwrap();
    let unregistered = KNOWN_UNREGISTERED.parse().unwrap();

    assert_eq!(
        engine.is_executable(&cgt),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&core),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&dual),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&recall),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&superposition),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&network),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&optimization),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&rewrite_normalize),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&surreal),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&surcomplex),
        cfg!(feature = "standard-probes")
    );
    assert_eq!(
        engine.is_executable(&viterbi),
        cfg!(feature = "standard-probes")
    );
    assert!(!engine.is_executable(&unregistered));
    assert_eq!(
        engine.executable_probe_ids(),
        if cfg!(feature = "standard-probes") {
            vec![
                cgt,
                core,
                dual,
                recall,
                superposition,
                network,
                optimization,
                rewrite_normalize,
                surcomplex,
                surreal,
                viterbi,
            ]
        } else {
            Vec::new()
        }
    );
}

#[test]
fn unknown_and_known_unregistered_probes_are_typed_errors() {
    let engine = ProbeEngine::new().unwrap();
    let unknown = "amari-probe:unknown:operation:v1".parse().unwrap();
    let known = KNOWN_UNREGISTERED.parse().unwrap();

    assert!(matches!(
        engine.execute(&unknown, &json!({})),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("unknown probe")
    ));
    assert!(matches!(
        engine.execute(&known, &json!({})),
        Err(DiscoveryError::ProbeUnavailable(message)) if message.contains(KNOWN_UNREGISTERED)
    ));
}

#[cfg(feature = "standard-probes")]
#[test]
fn execution_reports_cooperative_isolation_and_is_byte_deterministic() {
    let engine = ProbeEngine::new().unwrap();
    let id = VITERBI.parse().unwrap();
    let input = serde_json::to_value(request()).unwrap();

    let first = engine.execute(&id, &input).unwrap();
    let second = engine.execute(&id, &input).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.probe_id, id);
    assert_eq!(first.isolation, ProbeIsolation::Cooperative);
    assert_eq!(first.backend, amari_discovery::ProbeBackend::Cpu);
    assert!(first.deterministic);
    assert_eq!(
        first.input_schema,
        "amari.discovery/probe/tropical-viterbi/input/v1"
    );
    assert_eq!(
        first.output_schema,
        "amari.discovery/probe/tropical-viterbi/output/v1"
    );
    assert!(first.resources.operations > 0);
    assert!(first.resources.nodes > 0);
    assert!(first.resources.iterations > 0);
    assert!(first.resources.bytes > 0);
}

#[cfg(not(feature = "standard-probes"))]
#[test]
fn no_default_features_keeps_descriptor_known_but_not_executable() {
    let engine = ProbeEngine::new().unwrap();
    let id = VITERBI.parse().unwrap();

    assert!(!engine.is_executable(&id));
    assert!(matches!(
        engine.execute(&id, &serde_json::to_value(request()).unwrap()),
        Err(DiscoveryError::ProbeUnavailable(_))
    ));
}
