#![cfg(feature = "holographic")]
use amari_fusion::{unbind_general, BindingAlgebra, MAPAlgebra, TypedKey};

#[test]
fn fusion_distinguishes_typed_keys_from_general_inverse() {
    let value = MAPAlgebra::new([2., -3.]);
    assert!(TypedKey::try_new(value.clone()).is_err());
    let key = TypedKey::try_new(MAPAlgebra::new([-1., 1.])).unwrap();
    assert_eq!(
        key.unbind_key(&key.bind_key(&value).unwrap())
            .unwrap()
            .components(),
        &[2., -3.]
    );
    let bound = value.bind(&MAPAlgebra::new([3., 1.]));
    assert_eq!(
        unbind_general(&value, &bound).unwrap().components(),
        &[3., 1.]
    );
}
