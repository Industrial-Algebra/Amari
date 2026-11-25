//! Complete representations for memory and resonator certificates. In particular,
//! FHRR's legacy real projection is not a complete representation.
use super::{HolographicError, HolographicResult};
use crate::algebra::{BindingAlgebra, Cl3, CliffordAlgebra, FHRRAlgebra, MAPAlgebra, ProductCliffordAlgebra};
use alloc::vec::Vec;

/// A backend explicitly admitting its complete representation to checked memory.
pub trait MemoryElement: BindingAlgebra {
    /// Every independent real component, in stable order (complex bins interleaved).
    fn full_coefficients(&self) -> Vec<f64>;
    /// Backend-specific admissibility before certifying both inverse products.
    fn admissible_key(&self) -> bool { true }
}
impl MemoryElement for Cl3 { fn full_coefficients(&self)->Vec<f64>{self.to_coefficients()} }
impl<const K:usize> MemoryElement for ProductCliffordAlgebra<K> { fn full_coefficients(&self)->Vec<f64>{self.to_coefficients()} }
impl<const P:usize,const Q:usize,const R:usize> MemoryElement for CliffordAlgebra<P,Q,R> { fn full_coefficients(&self)->Vec<f64>{self.to_coefficients()} }
impl<const D:usize> MemoryElement for MAPAlgebra<D> {
    fn full_coefficients(&self)->Vec<f64>{self.to_coefficients()}
    fn admissible_key(&self)->bool{self.components().iter().all(|x| x.abs()==1.)}
}
impl<const D:usize> MemoryElement for FHRRAlgebra<D> {
    fn full_coefficients(&self)->Vec<f64>{(0..D).flat_map(|i| [self.real(i).unwrap(),self.imag(i).unwrap()]).collect()}
    fn admissible_key(&self)->bool{(0..D).all(|i| (self.real(i).unwrap().hypot(self.imag(i).unwrap())-1.).abs()<=1e-12)}
}
pub(crate) fn finite<A:MemoryElement>(x:&A)->HolographicResult<Vec<f64>> {
    let c=x.full_coefficients(); if c.is_empty() || c.iter().any(|v|!v.is_finite()) {return Err(HolographicError::InvalidData);} Ok(c)
}
pub(crate) fn key_inverse<A:MemoryElement>(key:&A)->HolographicResult<A>{
    finite(key)?;
    if !key.admissible_key(){return Err(HolographicError::InvalidKey);}
    let inv=key.inverse().map_err(|source|HolographicError::Algebra{operation:"key inverse".into(),source})?;
    finite(&inv)?;
    let one=A::identity().full_coefficients();
    for side in [key.bind(&inv),inv.bind(key)] { let c=finite(&side)?; if c.len()!=one.len() || !c.iter().zip(&one).all(|(x,y)|(x-y).abs()<=1e-12){return Err(HolographicError::InvalidKey);} }
    Ok(inv)
}
