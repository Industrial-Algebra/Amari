#![cfg(feature = "enumerative")]

use std::collections::BTreeSet;

use amari_enumerative::{FixedPoint, Matroid, TorusWeights};
use amari_gpu::{
    EnumerativeGpuError, EnumerativeGpuOps, EnumerativeGpuResult, GpuCSMData, GpuIntersectionData,
    GpuLocalizationData, GpuMatroidRankData, GpuOperadData, GpuStabilityData, GpuWDVVData,
};

#[tokio::test]
async fn test_enumerative_public_api_high_use_paths() {
    let explicit_result: EnumerativeGpuResult<()> = Ok(());
    assert!(explicit_result.is_ok());
    let explicit_error = EnumerativeGpuError::Computation("expected".to_string());
    assert!(explicit_error.to_string().contains("expected"));

    assert_eq!(GpuWDVVData::from_degree(5).degree, 5);

    let mut gpu = match EnumerativeGpuOps::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let wdvv_data: Vec<GpuWDVVData> = (1..=6).map(GpuWDVVData::from_degree).collect();
    let counts = gpu.batch_wdvv_curve_counts(&wdvv_data).await.unwrap();
    assert_eq!(counts, vec![1, 1, 12, 620, 87_304, 26_312_976]);
    assert!(gpu.batch_wdvv_curve_counts(&[]).await.unwrap().is_empty());

    let intersection_data = vec![
        GpuIntersectionData {
            degree1: 3.0,
            degree2: 4.0,
            codimension1: 1.0,
            codimension2: 1.0,
            ambient_dimension: 2.0,
            genus_correction: 0.0,
            multiplicity_factor: 1.0,
            padding: 0.0,
        },
        GpuIntersectionData {
            degree1: 3.0,
            degree2: 4.0,
            codimension1: 2.0,
            codimension2: 2.0,
            ambient_dimension: 3.0,
            genus_correction: 0.0,
            multiplicity_factor: 1.0,
            padding: 0.0,
        },
    ];
    let intersections = gpu
        .batch_intersection_numbers(&intersection_data)
        .await
        .unwrap();
    assert_eq!(intersections.len(), 2);
    assert!((intersections[0] - 12.0).abs() < 1e-5);
    assert_eq!(intersections[1], 0.0);

    let weights = TorusWeights {
        weights: vec![1, 2, 3, 4],
    };
    let loc_data = vec![
        GpuLocalizationData::from_fixed_point(
            &FixedPoint {
                subset: vec![0, 1],
                grassmannian: (2, 4),
            },
            &weights,
        ),
        GpuLocalizationData::from_fixed_point(
            &FixedPoint {
                subset: vec![0, 2],
                grassmannian: (2, 4),
            },
            &weights,
        ),
    ];
    let euler = gpu
        .batch_localization_euler_classes(&loc_data)
        .await
        .unwrap();
    assert_eq!(euler.len(), 2);
    assert!((euler[0] - 12.0).abs() < 1e-5);
    assert!((euler[1] + 3.0).abs() < 1e-5);

    let matroid = Matroid::uniform(2, 4);
    let subset_full: BTreeSet<usize> = [0, 1, 2, 3].into_iter().collect();
    let subset_pair: BTreeSet<usize> = [0, 1].into_iter().collect();
    let subset_single: BTreeSet<usize> = [2].into_iter().collect();
    let matroid_data = vec![
        GpuMatroidRankData::from_matroid_subset(&matroid, &subset_full),
        GpuMatroidRankData::from_matroid_subset(&matroid, &subset_pair),
        GpuMatroidRankData::from_matroid_subset(&matroid, &subset_single),
    ];
    let ranks = gpu.batch_matroid_ranks(&matroid_data).await.unwrap();
    assert_eq!(ranks, vec![2, 2, 1]);

    let csm = gpu
        .batch_csm_euler_characteristics(&[
            GpuCSMData::from_partition(&[2, 1], (2, 4)),
            GpuCSMData::from_partition(&[2, 2], (2, 4)),
        ])
        .await
        .unwrap();
    assert_eq!(csm, vec![1, 1]);

    let operad = gpu
        .batch_operad_multiplicities(&[
            GpuOperadData {
                output_codimension: 1,
                input_codimension: 1,
                grassmannian_k: 2,
                grassmannian_n: 4,
            },
            GpuOperadData {
                output_codimension: 1,
                input_codimension: 2,
                grassmannian_k: 2,
                grassmannian_n: 4,
            },
        ])
        .await
        .unwrap();
    assert_eq!(operad, vec![1, 0]);

    let stability_data = vec![
        GpuStabilityData {
            codimension: 1.0,
            dimension: 4.0,
            trust_level: 1.0,
            padding: 0.0,
        },
        GpuStabilityData {
            codimension: 1.0,
            dimension: 4.0,
            trust_level: 0.0,
            padding: 0.0,
        },
    ];
    let phases = gpu.batch_stability_phases(&stability_data).await.unwrap();
    assert_eq!(phases.len(), 2);
    assert!(phases[0] > 0.0 && phases[0] < 1.0);
    assert_eq!(phases[1], 1.0);

    let checks = gpu.batch_stability_checks(&stability_data).await.unwrap();
    assert_eq!(checks, vec![1, 0]);
}
