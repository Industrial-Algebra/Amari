#![cfg(feature = "holographic")]
use amari_gpu::GpuHolographicMemory;
use amari_holographic::{BindingAlgebra, ProductCliffordAlgebra};
type ProductCl3x32 = ProductCliffordAlgebra<32>;

#[tokio::test]
async fn h3_gpu_memory_batch_is_atomic() {
    let mut memory = GpuHolographicMemory::new().await.unwrap();
    let key = ProductCl3x32::identity();
    let values = [key.clone(), key.clone()];
    let bad = [key.clone(), ProductCl3x32::zero()];
    assert!(
        memory.store_batch(&bad, &values).await.is_err(),
        "singular final key must reject entire batch"
    );
    assert_eq!(memory.capacity_info().item_count, 0);
    memory.store_batch(&values, &values).await.unwrap();
    assert_eq!(memory.capacity_info().item_count, 2);
    let raw = memory.retrieve(&key).unwrap().raw_value;
    assert_eq!(
        raw.to_coefficients(),
        key.to_coefficients()
            .iter()
            .map(|c| 2.0 * c)
            .collect::<Vec<_>>()
    );
    assert!(memory.retrieve(&ProductCl3x32::zero()).is_err());
}
