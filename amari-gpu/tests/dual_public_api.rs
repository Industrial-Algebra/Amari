#![cfg(feature = "dual")]

use amari_dual::DualNumber;
use amari_gpu::{DualGpuError, DualGpuOps, DualGpuResult, DualOperation, GpuDualNumber};

fn apply_cpu(mut value: DualNumber<f32>, operations: &[DualOperation]) -> DualNumber<f32> {
    for operation in operations {
        value = match operation {
            DualOperation::Sin => value.sin(),
            DualOperation::Cos => value.cos(),
            DualOperation::Exp => value.exp(),
            DualOperation::Log => value.ln(),
            DualOperation::ReLU => {
                if value.real > 0.0 {
                    value
                } else {
                    DualNumber::new(0.0, 0.0)
                }
            }
            DualOperation::Sigmoid => {
                let sig = 1.0 / (1.0 + (-value.real).exp());
                DualNumber::new(sig, value.dual * sig * (1.0 - sig))
            }
            DualOperation::Tanh => {
                let tanh = value.real.tanh();
                DualNumber::new(tanh, value.dual * (1.0 - tanh * tanh))
            }
            DualOperation::Square => value * value,
            DualOperation::Sqrt => value.sqrt(),
            DualOperation::Add | DualOperation::Multiply => {
                unreachable!("unsupported binary operations are tested separately")
            }
        };
    }
    value
}

#[tokio::test]
async fn test_dual_public_api_paths() {
    let dual = DualNumber::new(2.0_f32, 3.0_f32);
    let gpu_dual: GpuDualNumber = dual.into();
    assert_eq!(gpu_dual.real, 2.0);
    assert_eq!(gpu_dual.dual, 3.0);
    let roundtrip: DualNumber<f32> = gpu_dual.into();
    assert_eq!(roundtrip.real, dual.real);
    assert_eq!(roundtrip.dual, dual.dual);

    let explicit_result: DualGpuResult<()> = Ok(());
    assert!(explicit_result.is_ok());
    let explicit_error = DualGpuError::InvalidOperation("expected".to_string());
    assert!(explicit_error.to_string().contains("expected"));

    let mut gpu = match DualGpuOps::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let empty = gpu
        .batch_forward_ad(&[], &[DualOperation::Square])
        .await
        .unwrap();
    assert!(empty.is_empty());

    let inputs = vec![
        DualNumber::new(0.25_f32, 1.0_f32),
        DualNumber::new(0.75_f32, 2.0_f32),
        DualNumber::new(1.25_f32, -1.0_f32),
    ];
    let operations = vec![
        DualOperation::Square,
        DualOperation::Exp,
        DualOperation::Tanh,
    ];
    let actual = gpu.batch_forward_ad(&inputs, &operations).await.unwrap();
    assert_eq!(actual.len(), inputs.len());

    for (actual, input) in actual.iter().zip(inputs.iter()) {
        let expected = apply_cpu(*input, &operations);
        assert!((actual.real - expected.real).abs() < 1e-5);
        assert!((actual.dual - expected.dual).abs() < 1e-5);
    }

    let unsupported = gpu
        .batch_forward_ad(&inputs, &[DualOperation::Add])
        .await
        .unwrap_err();
    assert!(unsupported.to_string().contains("Add and Multiply"));
}
