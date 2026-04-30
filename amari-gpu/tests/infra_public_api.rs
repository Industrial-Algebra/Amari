mod common;

use amari_core::Multivector;
use amari_gpu::performance::DispatchBenchmark;
use amari_gpu::{
    AdaptiveDispatchPolicy, AdaptiveVerificationError, AdaptiveVerificationLevel, AdaptiveVerifier,
    CalibrationResult, DeviceId, GpuDispatcher, GpuOperationParams, GpuParam, GpuTimelineAnalyzer,
    PlatformCapabilities, RecommendationPriority, TimelineEvent, UnifiedGpuError,
    VerificationPlatform, VerifiedMultivector, WorkgroupOptimizer,
};
use common::direct_gpu_runtime_available;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[test]
fn test_timeline_public_api_baselines() {
    let mut event = TimelineEvent::new(
        "op-1".to_string(),
        DeviceId(7),
        "matrix_multiply".to_string(),
        12.5,
        (16, 16, 1),
        vec![1024, 2048],
    );
    assert_eq!(event.event_id, "op-1");
    assert!(event.cpu_duration().is_none());
    assert_eq!(event.memory_bandwidth_gb_s(), 0.0);
    event.add_metadata("domain".to_string(), "core".to_string());
    event.set_gpu_timestamps_for_test(20, 10);
    assert_eq!(event.gpu_duration_ns(), None);
    event.set_gpu_timestamps_for_test(10, 20);
    assert_eq!(event.gpu_duration_ns(), Some(10));
    event.complete();
    assert!(event.cpu_duration().is_some());

    let analyzer = GpuTimelineAnalyzer::new(2);
    analyzer.record_event(event.clone());
    analyzer.record_event(TimelineEvent::new(
        "op-2".to_string(),
        DeviceId(7),
        "vector_operation".to_string(),
        1.0,
        (64, 1, 1),
        vec![512],
    ));
    analyzer.record_event(TimelineEvent::new(
        "op-3".to_string(),
        DeviceId(8),
        "vector_operation".to_string(),
        1.0,
        (64, 1, 1),
        vec![512],
    ));

    let device_events = analyzer.get_device_events(DeviceId(7), None);
    assert_eq!(device_events.len(), 1); // max_events=2 evicted op-1
    let zero_window = analyzer.analyze_gpu_utilization(Duration::ZERO);
    assert!(zero_window.device_stats.is_empty());
    let bottlenecks = analyzer.detect_bottlenecks(Duration::from_secs(1));
    assert_eq!(bottlenecks.analysis_window, Duration::from_secs(1));
}

trait TimelineEventTestExt {
    fn set_gpu_timestamps_for_test(&mut self, start: u64, end: u64);
}

impl TimelineEventTestExt for TimelineEvent {
    fn set_gpu_timestamps_for_test(&mut self, start: u64, end: u64) {
        self.gpu_timestamp_start = Some(start);
        self.gpu_timestamp_end = Some(end);
    }
}

#[tokio::test]
async fn test_adaptive_verifier_public_cpu_forced_baseline() {
    std::env::set_var("AMARI_GPU_FORCE_CPU", "1");
    let mut verifier =
        AdaptiveVerifier::with_config(AdaptiveVerificationLevel::Balanced, Duration::from_secs(1))
            .await
            .unwrap();
    std::env::remove_var("AMARI_GPU_FORCE_CPU");

    assert!(matches!(
        verifier.platform(),
        VerificationPlatform::NativeCpu { .. }
    ));
    assert_eq!(
        verifier.verification_level(),
        &AdaptiveVerificationLevel::Balanced
    );
    assert_eq!(verifier.performance_budget(), Duration::from_secs(1));
    assert!(!verifier.should_use_gpu(10_000));

    let e1 = VerifiedMultivector::new(Multivector::<3, 0, 0>::basis_vector(0));
    let e2 = VerifiedMultivector::new(Multivector::<3, 0, 0>::basis_vector(1));
    let product = verifier.verified_geometric_product(&e1, &e2).await.unwrap();
    assert_eq!(
        product.inner().to_vec(),
        e1.inner().geometric_product(e2.inner()).to_vec()
    );

    let batch = verifier
        .verified_batch_geometric_product(std::slice::from_ref(&e1), std::slice::from_ref(&e2))
        .await
        .unwrap();
    assert_eq!(batch.len(), 1);
    assert!(matches!(
        verifier
            .verified_batch_geometric_product(std::slice::from_ref(&e1), &[])
            .await,
        Err(AdaptiveVerificationError::NoSuitableStrategy)
    ));
}

#[test]
fn test_platform_capabilities_public_api() {
    let platform = VerificationPlatform::NativeCpu {
        features: amari_gpu::CpuFeatures {
            supports_simd: true,
            core_count: 4,
            cache_size_kb: 8192,
        },
    };
    assert_eq!(platform.max_batch_size(), 4000);
    assert!(platform.supports_concurrent_verification());
    let profile = platform.performance_characteristics();
    assert!(profile.compute_throughput_gflops > 0.0);
    let _strategy = platform.optimal_strategy(10);
}

#[tokio::test]
async fn test_performance_policy_and_optimizer_public_api() {
    let mut optimizer = WorkgroupOptimizer::new();
    assert_eq!(
        optimizer.get_optimal_config("matrix_multiply").size,
        (16, 16, 1)
    );
    assert_eq!(optimizer.get_optimal_config("unknown").size, (64, 1, 1));

    let best = optimizer
        .calibrate_operation("nan_safe", |config| {
            if config.size == (128, 1, 1) {
                1000.0
            } else {
                f32::NAN
            }
        })
        .await
        .unwrap();
    assert_eq!(best.size, (128, 1, 1));
    let results: &[CalibrationResult] = optimizer.get_calibration_results("nan_safe").unwrap();
    assert_eq!(results.len(), 6);
    assert!(results.iter().all(|r| r.throughput_gops.is_finite()));

    let mut policy = AdaptiveDispatchPolicy::new();
    assert!(!policy.should_use_gpu("op", 999));
    assert!(policy.should_use_gpu("op", 1000));
    policy.update_from_benchmark(DispatchBenchmark {
        operation_type: "op".to_string(),
        data_size: 1000,
        cpu_time_ms: 10.0,
        gpu_time_ms: 1.0,
        timestamp: Instant::now(),
    });
    assert!(policy.get_crossover_points().contains_key("op"));
    policy.update_from_benchmark(DispatchBenchmark {
        operation_type: "bad".to_string(),
        data_size: 1,
        cpu_time_ms: f32::NAN,
        gpu_time_ms: f32::NAN,
        timestamp: Instant::now(),
    });
    assert!(policy.get_crossover_points().contains_key("bad"));
}

#[tokio::test]
async fn test_unified_dispatcher_public_cpu_fallback_and_params() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let params = GpuOperationParams {
        params: HashMap::from([
            ("alpha".to_string(), GpuParam::Float(0.5)),
            ("name".to_string(), GpuParam::Buffer("buf".to_string())),
        ]),
        batch_size: 2,
        workgroup_size: (64, 1, 1),
    };
    assert_eq!(params.batch_size, 2);

    let mut dispatcher = GpuDispatcher::new().await.unwrap();
    let result = dispatcher
        .execute(
            1,
            |_ctx| Err(UnifiedGpuError::InvalidOperation("skip".to_string())),
            || 42usize,
        )
        .await;
    assert_eq!(result, 42);
}

#[test]
fn test_recommendation_priority_import_path() {
    let priority = RecommendationPriority::High;
    assert!(matches!(priority, RecommendationPriority::High));
}
