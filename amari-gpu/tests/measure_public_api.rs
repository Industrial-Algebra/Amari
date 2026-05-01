#![cfg(feature = "measure")]

use amari_gpu::{
    GpuIntegrator, GpuMonteCarloIntegrator, GpuMultidimIntegrator, GpuParametricDensity,
    GpuTropicalMeasure,
};

#[tokio::test]
async fn test_measure_public_api_paths() {
    if let Ok(integrator) = GpuIntegrator::new().await {
        let quadratic = integrator
            .integrate_uniform(0.0, 2.0, 10_000, 1)
            .await
            .unwrap();
        assert!((quadratic - 8.0 / 3.0).abs() < 0.01);

        let from_values = integrator
            .integrate_values(&[1.0, 2.0, 3.0, 4.0], 0.5)
            .await
            .unwrap();
        assert!((from_values - 5.0).abs() < 1e-6);

        let zero_samples = integrator.integrate_uniform(0.0, 1.0, 0, 0).await;
        assert!(zero_samples.is_err());
    }

    if let Ok(integrator) = GpuMonteCarloIntegrator::new().await {
        let linear = integrator.integrate(0.0, 2.0, 20_000, 42, 0).await.unwrap();
        let constant = integrator
            .integrate(0.0, 2.0, 20_000, 42, 99)
            .await
            .unwrap();

        assert!((linear - 2.0).abs() < 0.1);
        assert!((constant - 2.0).abs() < 0.01);
    }

    if let Ok(density) = GpuParametricDensity::new().await {
        let values = density.gaussian_batch(&[0.0, 1.0], 0.0, 1.0).await.unwrap();
        assert_eq!(values.len(), 2);

        let expected_at_zero = 1.0 / (2.0 * std::f32::consts::PI).sqrt();
        let expected_at_one = expected_at_zero * (-0.5_f32).exp();
        assert!((values[0] - expected_at_zero).abs() < 1e-5);
        assert!((values[1] - expected_at_one).abs() < 1e-5);

        assert!(density.gaussian_batch(&[0.0], 0.0, 0.0).await.is_err());
        assert!(density
            .gaussian_batch(&[], 0.0, 1.0)
            .await
            .unwrap()
            .is_empty());
    }

    if let Ok(tropical) = GpuTropicalMeasure::new().await {
        let values = [-2.0, 4.5, 1.0, 3.0];
        assert_eq!(tropical.supremum(&values).await.unwrap(), 4.5);
        assert_eq!(tropical.infimum(&values).await.unwrap(), -2.0);
        assert!(tropical.supremum(&[]).await.is_err());
        assert!(tropical.infimum(&[]).await.is_err());
    }

    if let Ok(multidim) = GpuMultidimIntegrator::new().await {
        let volume = multidim
            .monte_carlo_nd(&[(0.0, 2.0), (-1.0, 1.0), (2.0, 5.0)], 10_000, 7)
            .await
            .unwrap();
        assert!((volume - 12.0).abs() < 1e-6);
    }
}
