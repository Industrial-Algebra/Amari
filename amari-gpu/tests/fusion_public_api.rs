#![cfg(feature = "fusion")]

mod common;

use amari_gpu::{
    FusionGpuError, FusionGpuResult, GpuHolographicTDC, GpuResonatorOutput, HolographicGpuOps,
};
use common::direct_gpu_runtime_available;

#[test]
fn test_fusion_public_api_holographic_types() {
    let tdc = GpuHolographicTDC::default();
    assert_eq!(tdc.tropical, f32::NEG_INFINITY);
    assert_eq!(tdc.dual_real, 0.0);
    assert_eq!(tdc.dual_dual, 0.0);
    assert_eq!(tdc.clifford, [0.0; 8]);

    let output = GpuResonatorOutput {
        cleaned: tdc,
        best_index: 7,
        best_similarity: 0.875,
        _padding: [0.0; 2],
    };
    assert_eq!(output.best_index, 7);
    assert!((output.best_similarity - 0.875).abs() < 1e-6);
}

#[test]
fn test_fusion_public_api_threshold() {
    assert!(!HolographicGpuOps::should_use_gpu(99));
    assert!(HolographicGpuOps::should_use_gpu(100));
}

#[test]
fn test_fusion_public_error_and_result_types() {
    let result: FusionGpuResult<()> = Err(FusionGpuError::InvalidOperation(
        "public fusion API smoke error".to_string(),
    ));

    match result {
        Err(error) => assert!(error.to_string().contains("public fusion API smoke error")),
        Ok(_) => panic!("expected public fusion API smoke error"),
    }
}

#[tokio::test]
async fn test_fusion_public_api_empty_batch_bind_if_gpu_available() {
    if !direct_gpu_runtime_available() {
        return;
    }
    let ops = match HolographicGpuOps::new().await {
        Ok(ops) => ops,
        Err(_) => return,
    };

    let result = ops.batch_bind(&[], &[]).await.unwrap();
    assert!(result.is_empty());
}
