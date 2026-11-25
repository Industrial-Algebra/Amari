//! Public wire and arithmetic oracle for the additive MAP256 recall probe.
#![cfg(feature = "standard-probes")]
use amari_discovery::ProbeEngine;
use amari_holographic::algebra::map::MAP256;
use serde_json::json;

#[test]
fn h3_recall_v2_evidence_and_raw_coefficients() {
    let engine = ProbeEngine::new().unwrap();
    let probe = "amari-probe:holographic:recall:v2".parse().unwrap();
    let entries = json!([{"key_seed":1,"value_seed":7},{"key_seed":2,"value_seed":9}]);
    for query in [1, 3] {
        let out = engine
            .execute(&probe, &json!({"entries":entries,"query_seed":query}))
            .unwrap()
            .output;
        assert!(out.get("confidence").is_none());
        let evidence = &out["membership_evidence"];
        if query == 1 {
            assert_eq!(evidence, &json!({"kind":"present","indices":[0]}));
        } else {
            assert_eq!(evidence, &json!({"kind":"absent"}));
        }
        assert!(out["nearest_key_similarity"].as_f64().is_some());
        let [q, a, b, c, d] = [query, 1, 7, 2, 9].map(MAP256::from_seed);
        let expected: Vec<_> = (0..256)
            .map(|i| {
                q.components()[i]
                    * (a.components()[i] * b.components()[i]
                        + c.components()[i] * d.components()[i])
            })
            .collect();
        assert_eq!(out["raw_coefficients"], json!(expected));
    }
}
