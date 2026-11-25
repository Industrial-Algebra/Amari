use amari_holographic::{BindingAlgebra, Cl3, HolographicMemory, MAPAlgebra, FHRRAlgebra};

#[test]
fn u2_transactional_bad_prefix_and_mixed_tracking() {
 let one=Cl3::one(); let key=Cl3::scalar(2.)+Cl3::unit_vector(0); let v=Cl3::unit_vector(1);
 let mut m=HolographicMemory::with_key_tracking(Default::default()); m.store(&key,&v).unwrap(); let before=*m.trace();
 assert!(m.store_batch(&[(one,v),(one+Cl3::unit_vector(0),v)]).is_err(),"batch accepted a zero divisor");
 assert_eq!(*m.trace(),before); assert_eq!(m.item_count(),1); assert!(m.verify_consistency());
 let mut untracked=HolographicMemory::new(Default::default()); m.merge(&untracked).unwrap(); untracked.store(&one,&Cl3::zero()).unwrap();
 assert!(m.merge(&untracked).is_err()); assert!(untracked.merge(&m).is_err()); assert_eq!(*m.trace(),before); assert_eq!(m.item_count(),1);
}
#[test]
fn u2_invalid_key_classes_and_imaginary_values() {
 type M=MAPAlgebra<2>; type F=FHRRAlgebra<2>;
 let mut m=HolographicMemory::new(Default::default()); assert!(m.store(&M::new([2.,1.]),&M::identity()).is_err(),"accepted non-bipolar key");
 let mut f=HolographicMemory::new(Default::default()); assert!(f.store(&F::new([1.,0.],[0.,0.]),&F::identity()).is_err()); assert!(f.store(&F::identity(),&F::new([0.;2],[0.,f64::NAN])).is_err()); assert_eq!(f.item_count(),0);
}
