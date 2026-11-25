//! Dedicated Vulkan runtime gate. Invoked explicitly on a lavapipe-equipped runner.
#![cfg(feature = "holographic")]
use amari_gpu::GpuHolographic;

const MASK: [usize; 8] = [0, 1, 2, 4, 3, 5, 6, 7];
// Independent ordered-word oracle; do not import the CPU or shader tables.
fn mul(a: &[f64], b: &[f64]) -> [f64; 8] {
    let mut r = [0.; 8];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            let mut sign = 1.;
            for m in 0..3 {
                for n in 0..m {
                    if MASK[i] & (1 << m) != 0 && MASK[j] & (1 << n) != 0 {
                        sign = -sign;
                    }
                }
            }
            let k = MASK.iter().position(|&v| v == MASK[i] ^ MASK[j]).unwrap();
            r[k] += sign * x * y;
        }
    }
    r
}
#[tokio::test]
#[ignore = "required in Holographic Shader Parity CI job (needs Vulkan)"]
async fn h1_shader_parity() {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN);
    assert!(
        !adapters.is_empty(),
        "Vulkan adapter enumeration is required"
    );
    for a in adapters {
        eprintln!("Enumerated {:?}", a.get_info());
    }
    let gpu = GpuHolographic::new_product_cl3x32()
        .await
        .expect("device and bind pipelines required");
    eprintln!("Selected adapter: {:?}", gpu.adapter_info());
    assert_eq!(gpu.adapter_info().backend, wgpu::Backend::Vulkan);
    for batch in [4, 101] {
        let mut keys = vec![0.; batch * 256];
        let mut values = keys.clone();
        let mut expected = keys.clone();
        for (n, ((a, b), out)) in keys
            .as_chunks_mut::<8>()
            .0
            .iter_mut()
            .zip(values.as_chunks_mut::<8>().0.iter_mut())
            .zip(expected.as_chunks_mut::<8>().0.iter_mut())
            .enumerate()
        {
            if n % 128 < 64 {
                let p = n % 64;
                a[p / 8] = 1.;
                b[p % 8] = 1.;
            } else if n % 3 != 0 {
                a[0] = 0.37;
                a[n % 7 + 1] = 0.21;
                b[0] = -0.78;
                b[(n + 2) % 7 + 1] = 0.15;
            }
            out.copy_from_slice(&mul(a, b));
        }
        let result = gpu.batch_bind(&keys, &values).await.unwrap();
        assert_eq!(result.len(), expected.len());
        for (n, (&v, &w)) in result.iter().zip(&expected).enumerate() {
            let tol = if n / 8 % 128 < 64 { 0. } else { 1e-6 };
            assert!(
                (v - w).abs() <= tol,
                "batch={batch} coeff={n} actual={v} expected={w}"
            );
        }
        eprintln!(
            "readback OK: {batch} records / {} coefficients; shader={}",
            result.len(),
            GpuHolographic::should_use_gpu(batch)
        );
    }
    let key = [2., 1., 0., 0., 0., 0., 0., 0.].repeat(32);
    let value = [0., 0., 1., 0., 0., 0., 0., 0.].repeat(32);
    let bound = gpu.batch_bind(&key, &value).await.unwrap();
    let rec = gpu.batch_unbind(&key, &bound).await.unwrap();
    for (v, w) in rec.iter().zip(value) {
        assert!((v - w).abs() <= 1e-12);
    }
    let mut singular = key;
    singular[0] = 1.;
    assert!(gpu.batch_unbind(&singular, &bound).await.is_err());
}
