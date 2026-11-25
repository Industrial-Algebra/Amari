use amari_holographic::{BindingAlgebra,FHRRAlgebra, HolographicMemory, MAPAlgebra, memory::MembershipEvidence};
#[test] fn u3_full_exact_evidence_and_duplicate_indices(){
 type F=FHRRAlgebra<1>;let pos=F::new([0.],[1.]);let neg=F::new([0.],[-1.]);let mut t=HolographicMemory::<F>::with_key_tracking(Default::default());
 assert_eq!(t.membership(&pos).unwrap(),MembershipEvidence::Absent);assert!(t.nearest_key(&pos).unwrap().is_none());
 t.store(&pos,&pos).unwrap();t.store(&pos,&pos).unwrap();assert_eq!(t.membership(&pos).unwrap(),MembershipEvidence::Present{indices:vec![0,1]});assert_eq!(t.membership(&neg).unwrap(),MembershipEvidence::Absent);assert_eq!(t.nearest_key(&neg).unwrap().unwrap().similarity,-1.);
 let almost=F::new([1e-12],[1.]); assert_eq!(t.membership(&almost).unwrap(),MembershipEvidence::Absent);assert!(t.nearest_key(&almost).unwrap().unwrap().similarity>0.99);
 let u=HolographicMemory::<F>::new(Default::default());assert_eq!(u.membership(&pos).unwrap(),MembershipEvidence::Unknown);assert!(u.nearest_key(&pos).unwrap().is_none());
}
#[test] fn u3_walsh_crosstalk_without_age_gradient(){type M=MAPAlgebra<8>;let keys:[M;8]=std::array::from_fn(|i| M::new(std::array::from_fn(|j| if(i&j).count_ones()%2==0 {1.}else{-1.})));let mut m=HolographicMemory::with_key_tracking(Default::default()); for k in &keys{m.store(k,&M::identity()).unwrap();}for(i,k)in keys.iter().enumerate(){let raw=m.retrieve(k).unwrap().raw_value;assert_eq!(raw.components(),&[8.,0.,0.,0.,0.,0.,0.,0.]);assert!((raw.similarity(&M::identity())-1./8f64.sqrt()).abs()<1e-12);assert_eq!(m.membership(k).unwrap(),MembershipEvidence::Present{indices:vec![i]});}}
