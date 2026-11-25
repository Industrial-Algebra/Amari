use amari_holographic::{BindingAlgebra, Cl3, CliffordAlgebra, Resonator};

fn sided<A: amari_holographic::memory::MemoryElement>(a: A, b: A) {
    let target = a.bind(&b);
    let left = Resonator::new(vec![a.clone(), a.scale(-1.).unwrap()], Default::default()).unwrap();
    let right = Resonator::new(vec![b.clone()], Default::default()).unwrap();
    let result = left.factorize(&target, &right).unwrap();
    assert!(result.accepted);
    assert_eq!(result.factor_a.bind(&result.factor_b).to_coefficients(), target.to_coefficients());
    assert_eq!(result.factor_a.to_coefficients(), a.to_coefficients());
    assert_eq!(result.factor_b.to_coefficients(), b.to_coefficients());
}
#[test]
fn h3_sided_local() { sided(Cl3::vector(1.,0.,0.), Cl3::vector(0.,1.,0.)); }
#[test]
fn h3_sided_core_wrapper() {
    type C = CliffordAlgebra<3,0,0>;
    sided(C::unit_vector(0).unwrap(), C::unit_vector(1).unwrap());
}

#[test] fn h3_impossible_and_singular(){let a=Cl3::unit_vector(0);let b=Cl3::unit_vector(1);let l=Resonator::new(vec![a],Default::default()).unwrap();let r=Resonator::new(vec![b],Default::default()).unwrap();assert!(!l.factorize(&(a*b).scale(2.),&r).unwrap().accepted);let s=Resonator::new(vec![Cl3::one()+a],Default::default()).unwrap();assert!(l.factorize(&(a*b),&s).is_err());}
