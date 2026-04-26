#![cfg(feature = "tropical")]

use amari_gpu::{TropicalExecutionPath, TropicalGpuOps};
use amari_tropical::{TropicalMatrix, TropicalNumber};

fn cpu_tropical_matmul(a: &TropicalMatrix<f32>, b: &TropicalMatrix<f32>) -> TropicalMatrix<f32> {
    let mut out = TropicalMatrix::new(a.rows, b.cols);
    for i in 0..a.rows {
        for j in 0..b.cols {
            let mut max_val = f32::NEG_INFINITY;
            for k in 0..a.cols {
                max_val = max_val.max(a.data[i][k].value() + b.data[k][j].value());
            }
            out.data[i][j] = TropicalNumber::new(max_val);
        }
    }
    out
}

fn assert_matrix_close(a: &TropicalMatrix<f32>, b: &TropicalMatrix<f32>, tol: f32) {
    assert_eq!(a.rows, b.rows);
    assert_eq!(a.cols, b.cols);
    for i in 0..a.rows {
        for j in 0..a.cols {
            assert!(
                (a.data[i][j].value() - b.data[i][j].value()).abs() < tol,
                "Mismatch at ({}, {}): left={}, right={}",
                i,
                j,
                a.data[i][j].value(),
                b.data[i][j].value()
            );
        }
    }
}

fn cpu_attention_scores(logits: &TropicalMatrix<f32>) -> TropicalMatrix<f32> {
    let mut scores = TropicalMatrix::new(logits.rows, logits.cols);
    for i in 0..logits.rows {
        let mut row_max = f32::NEG_INFINITY;
        for j in 0..logits.cols {
            row_max = row_max.max(logits.data[i][j].value());
        }
        for j in 0..logits.cols {
            let score = if logits.data[i][j].value() == row_max { 1.0 } else { 0.0 };
            scores.data[i][j] = TropicalNumber::new(score);
        }
    }
    scores
}

#[tokio::test]
async fn test_tropical_public_api_adaptive_matrix_multiply() {
    let mut gpu = match TropicalGpuOps::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let mut a = TropicalMatrix::new(16, 16);
    let mut b = TropicalMatrix::new(16, 16);

    for i in 0..16 {
        for j in 0..16 {
            a.data[i][j] = TropicalNumber::new((i as f32 - j as f32) * 0.25);
            b.data[i][j] = TropicalNumber::new((i as f32 + j as f32) * 0.125);
        }
    }

    let path = gpu.matrix_multiply_execution_path(a.rows, a.cols, b.cols);
    assert_eq!(path, TropicalExecutionPath::Cpu);

    let adaptive = gpu.matrix_multiply_adaptive(&a, &b).await.unwrap();
    let cpu = cpu_tropical_matmul(&a, &b);
    assert_matrix_close(&adaptive, &cpu, 1e-5);
}

#[tokio::test]
async fn test_tropical_public_api_attention_scores() {
    let mut gpu = match TropicalGpuOps::new().await {
        Ok(gpu) => gpu,
        Err(_) => return,
    };

    let mut logits = TropicalMatrix::new(2, 4);
    logits.data[0][0] = TropicalNumber::new(0.0);
    logits.data[0][1] = TropicalNumber::new(2.0);
    logits.data[0][2] = TropicalNumber::new(1.0);
    logits.data[0][3] = TropicalNumber::new(2.0);
    logits.data[1][0] = TropicalNumber::new(-4.0);
    logits.data[1][1] = TropicalNumber::new(-3.0);
    logits.data[1][2] = TropicalNumber::new(-2.0);
    logits.data[1][3] = TropicalNumber::new(-1.0);

    let gpu_scores = gpu.attention_scores(&logits).await.unwrap();
    let cpu_scores = cpu_attention_scores(&logits);
    assert_matrix_close(&gpu_scores, &cpu_scores, 1e-6);
}
