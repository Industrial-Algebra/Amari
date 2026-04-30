#![cfg(feature = "calculus")]

use amari_calculus::{vector_from_slice, ScalarField, VectorField};
use amari_gpu::GpuCalculus;

#[tokio::test]
async fn test_calculus_public_api_large_batch_preserves_scalar_cpu_semantics() {
    let gpu = match GpuCalculus::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let field =
        ScalarField::<3, 0, 0>::new(|coords| coords[0] * coords[0] + 2.0 * coords[1] + coords[2]);
    let points: Vec<[f64; 3]> = (0..1000).map(|i| [i as f64 * 0.001, 1.0, -0.5]).collect();

    let values = gpu.batch_eval_scalar_field(&field, &points).await.unwrap();
    assert_eq!(values.len(), points.len());
    for (value, point) in values.iter().zip(points.iter()) {
        let expected = field.evaluate(point);
        assert!((value - expected).abs() < 1e-12);
    }
}

#[tokio::test]
async fn test_calculus_public_api_large_batch_preserves_vector_cpu_semantics() {
    let gpu = match GpuCalculus::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let field = VectorField::<3, 0, 0>::new(|coords| {
        vector_from_slice(&[coords[0], 2.0 * coords[1], -coords[2]])
    });
    let points: Vec<[f64; 3]> = (0..1000).map(|i| [i as f64 * 0.001, 2.0, -3.0]).collect();

    let values = gpu.batch_eval_vector_field(&field, &points).await.unwrap();
    assert_eq!(values.len(), points.len());
    for (value, point) in values.iter().zip(points.iter()) {
        let expected = field.evaluate(point);
        for component in 0..3 {
            assert!(
                (value.vector_component(component) - expected.vector_component(component)).abs()
                    < 1e-12
            );
        }
    }
}
