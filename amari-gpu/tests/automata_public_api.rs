#![cfg(feature = "automata")]

mod common;

use amari_gpu::{
    AutomataGpuConfig, AutomataGpuError, AutomataGpuOps, AutomataGpuResult, GpuCellData,
    GpuEvolutionParams, GpuRuleConfig,
};
use common::direct_gpu_runtime_available;

fn sample_cells() -> Vec<GpuCellData> {
    vec![
        GpuCellData {
            scalar: 2.0,
            e1: 3.0,
            e2: 4.0,
            generation: 7.0,
            ..GpuCellData::default()
        },
        GpuCellData {
            scalar: 0.4,
            e1: -2.0,
            e2: 0.5,
            generation: 7.0,
            ..GpuCellData::default()
        },
        GpuCellData {
            scalar: 1.0,
            e1: 0.0,
            e2: -1.0,
            generation: 7.0,
            ..GpuCellData::default()
        },
        GpuCellData {
            scalar: -2.0,
            e1: 1.0,
            e2: 2.0,
            generation: 7.0,
            ..GpuCellData::default()
        },
    ]
}

#[tokio::test]
async fn test_automata_public_api_paths() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let config = AutomataGpuConfig::default();
    assert_eq!(config.workgroup_size, (16, 16, 1));

    let explicit_result: AutomataGpuResult<()> = Ok(());
    assert!(explicit_result.is_ok());
    let explicit_error = AutomataGpuError::EvolutionComputationFailed("expected".to_string());
    assert!(explicit_error.to_string().contains("expected"));

    let mut ops = match AutomataGpuOps::new().await {
        Ok(ops) => ops,
        Err(_) => return,
    };

    let cells = sample_cells();
    let rule = GpuRuleConfig {
        damping_factor: 0.1,
        threshold: 0.5,
        ..GpuRuleConfig::default()
    };

    let applied = ops.batch_apply_rules(&cells, &[rule]).await.unwrap();
    assert_eq!(applied.len(), cells.len());
    assert!((applied[0].scalar - 1.8).abs() < 1e-5);
    assert!((applied[0].e1 - 2.7).abs() < 1e-5);
    assert_eq!(applied[1].scalar, 0.0);
    assert!((applied[1].e1 + 1.8).abs() < 1e-5);

    assert!(ops.batch_apply_rules(&cells, &[]).await.is_err());
    assert!(ops
        .batch_apply_rules(&[], &[rule])
        .await
        .unwrap()
        .is_empty());

    let energy = ops.calculate_total_energy(&cells).await.unwrap();
    let expected_energy: f32 = cells
        .iter()
        .map(|cell| {
            cell.scalar * cell.scalar
                + cell.e1 * cell.e1
                + cell.e2 * cell.e2
                + cell.e3 * cell.e3
                + cell.e12 * cell.e12
                + cell.e13 * cell.e13
                + cell.e23 * cell.e23
                + cell.e123 * cell.e123
        })
        .sum();
    assert!((energy - expected_energy).abs() < 1e-5);
    assert_eq!(ops.calculate_total_energy(&[]).await.unwrap(), 0.0);

    let neighborhoods = ops.extract_neighborhoods(&cells, 2, 2).await.unwrap();
    assert_eq!(neighborhoods.len(), cells.len());
    assert_eq!(neighborhoods[0].len(), 8);
    assert!(neighborhoods[0]
        .iter()
        .any(|neighbor| (neighbor.scalar - cells[3].scalar).abs() < 1e-6));
    assert!(ops.extract_neighborhoods(&cells, 3, 2).await.is_err());

    let params = GpuEvolutionParams {
        steps_per_batch: 1.0,
        current_generation: 7.0,
        ..GpuEvolutionParams::default()
    };
    let evolved = ops.batch_evolve_ca(&cells, &[rule], &params).await.unwrap();
    assert_eq!(evolved.len(), cells.len());
    assert!(evolved
        .iter()
        .all(|cell| (cell.generation - 8.0).abs() < 1e-6));

    let bad_params = GpuEvolutionParams {
        steps_per_batch: -1.0,
        ..params
    };
    assert!(ops
        .batch_evolve_ca(&cells, &[rule], &bad_params)
        .await
        .is_err());
}
