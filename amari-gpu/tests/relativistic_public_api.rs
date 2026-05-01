use amari_gpu::{
    GpuError, GpuRelativisticParticle, GpuRelativisticPhysics, GpuSpacetimeVector,
    GpuTrajectoryParams,
};
use amari_relativistic::spacetime::SpacetimeVector;

fn assert_close(actual: f32, expected: f32, tol: f32) {
    assert!(
        (actual - expected).abs() <= tol,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn test_relativistic_public_conversions_and_errors() {
    let cpu = SpacetimeVector::from_coordinates([2.0, 1.0, 0.5, 0.25]);
    let gpu = GpuSpacetimeVector::from_spacetime_vector(&cpu);
    assert_close(gpu.t, 2.0, 1e-6);
    assert_close(gpu.x, 1.0, 1e-6);

    let roundtrip = gpu.to_spacetime_vector();
    assert_eq!(roundtrip.coordinates(), [2.0, 1.0, 0.5, 0.25]);

    let err = GpuError::BufferError("expected".to_string());
    assert!(err.to_string().contains("expected"));
}

#[tokio::test]
async fn test_relativistic_public_gpu_paths_when_available() {
    let gpu = match GpuRelativisticPhysics::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    assert!(gpu
        .compute_minkowski_products(&[])
        .await
        .unwrap()
        .is_empty());
    assert!(matches!(
        gpu.compute_minkowski_products(&[GpuSpacetimeVector::new(f32::NAN, 0.0, 0.0, 0.0)])
            .await,
        Err(GpuError::BufferError(_))
    ));

    let vectors = vec![
        GpuSpacetimeVector::new(2.0, 1.0, 0.5, 0.25),
        GpuSpacetimeVector::new(5.0, 3.0, 4.0, 0.0),
    ];
    let products = gpu.compute_minkowski_products(&vectors).await.unwrap();
    assert_eq!(products.len(), 2);
    assert_close(products[0], 4.0 - 1.0 - 0.25 - 0.0625, 1e-5);
    assert_close(products[1], 25.0 - 9.0 - 16.0, 1e-5);

    let particle = GpuRelativisticParticle {
        position: GpuSpacetimeVector::new(0.0, 10.0, 0.0, 0.0),
        velocity: GpuSpacetimeVector::new(1.0, 0.25, 0.0, 0.0),
        mass: 1.0,
        charge: 0.0,
        proper_time: 0.0,
        _padding: [0.0; 5],
    };
    let zero_step = GpuTrajectoryParams {
        dt: 0.1,
        steps: 0,
        tolerance: 1e-3,
        renorm_freq: 1,
        schwarzschild_radius: 0.0,
        gm_parameter: 0.0,
        _padding: [0.0; 2],
    };
    let unchanged = gpu
        .propagate_particles(&[particle], &zero_step)
        .await
        .unwrap();
    assert_eq!(unchanged.len(), 1);
    assert_close(unchanged[0].position.x, particle.position.x, 1e-6);

    let one_step = GpuTrajectoryParams {
        steps: 1,
        ..zero_step
    };
    let invalid_params = GpuTrajectoryParams {
        renorm_freq: 0,
        ..one_step
    };
    assert!(matches!(
        gpu.propagate_particles(&[particle], &invalid_params).await,
        Err(GpuError::BufferError(_))
    ));

    let invalid_particle = GpuRelativisticParticle {
        mass: -1.0,
        ..particle
    };
    assert!(matches!(
        gpu.propagate_particles(&[invalid_particle], &one_step)
            .await,
        Err(GpuError::BufferError(_))
    ));

    // With zero Schwarzschild radius the acceleration terms vanish for this
    // nonzero-radius particle, so the simplified geodesic kernel reduces to
    // position += velocity * dt and proper_time += dt.
    let propagated = gpu
        .propagate_particles(&[particle], &one_step)
        .await
        .unwrap();
    assert_eq!(propagated.len(), 1);
    assert_close(propagated[0].position.t, 0.1, 1e-5);
    assert_close(propagated[0].position.x, 10.025, 1e-5);
    assert_close(propagated[0].proper_time, 0.1, 1e-5);
}
