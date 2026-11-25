//! Additive, checked associative memory. A raw trace is an ordered sum of bindings.
use super::domain::{finite,key_inverse,MemoryElement};
use super::{HolographicError,HolographicResult};
use crate::algebra::{AlgebraConfig,BindingAlgebra};
use alloc::vec::Vec;

/// A raw additive trace with optional exact ordered key tracking.
#[derive(Clone)]
pub struct HolographicMemory<A:BindingAlgebra>{trace:A,item_count:usize,config:AlgebraConfig,stored_keys:Option<Vec<A>>}
/// Exact tracked membership, distinct from similarity.
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum MembershipEvidence { /// All matching record positions, in ingestion order.
Present { /// Exact matching indices.
indices:Vec<usize> }, /// No match among fully tracked keys.
Absent, /// Key tracking was disabled.
Unknown }
/// Best tracked coefficient cosine. Ties resolve to the earliest record.
#[derive(Clone,Debug,PartialEq)]
pub struct NearestKey { /// Storage position.
pub index:usize, /// Full-representation coefficient cosine (not a probability).
pub similarity:f64 }
/// Raw retrieval from an additive trace. No implicit normalization or cleanup.
#[derive(Clone,Debug)]
pub struct RetrievalResult<A:BindingAlgebra>{/// Left unbinding of the entire trace.
pub raw_value:A}
/// Nominal load heuristics, not query evidence or measured interference.
#[derive(Clone,Debug)]
pub struct CapacityInfo{/// Number of stored records.
pub item_count:usize,/// Nominal load scale.
pub theoretical_capacity:usize,/// Nominal signal-to-noise estimate from load alone.
pub estimated_snr:f64,/// Nominal load warning threshold.
pub snr_threshold:f64,/// Heuristic load warning, not retrieval confidence.
pub near_capacity:bool}

impl<A:MemoryElement> HolographicMemory<A>{
    /// Create empty, untracked memory.
    pub fn new(config:AlgebraConfig)->Self{Self{trace:A::zero(),item_count:0,config,stored_keys:None}}
    /// Create empty memory retaining ordered keys.
    pub fn with_key_tracking(config:AlgebraConfig)->Self{Self{stored_keys:Some(Vec::new()),..Self::new(config)}}
    /// Return the unmodified sum of bindings.
    pub fn trace(&self)->&A{&self.trace}
    /// Number of committed records.
    pub fn item_count(&self)->usize{self.item_count}
    /// Finite trace, with one tracked key per record when tracking is enabled.
    pub fn verify_consistency(&self)->bool{finite(&self.trace).is_ok() && self.stored_keys.as_ref().is_none_or(|keys|keys.len()==self.item_count)}
    /// Remove all associations, preserving the tracking mode.
    pub fn clear(&mut self){self.trace=A::zero();self.item_count=0;if let Some(keys)=&mut self.stored_keys{keys.clear();}}
    /// Nominal dimension/load heuristics only.
    pub fn capacity_info(&self)->CapacityInfo{let theoretical_capacity=self.trace.theoretical_capacity();CapacityInfo{item_count:self.item_count,theoretical_capacity,estimated_snr:if self.item_count==0{f64::INFINITY}else{self.trace.estimate_snr(self.item_count)},snr_threshold:2.,near_capacity:self.item_count>theoretical_capacity/2}}
    fn add(trace:&A,bound:&A)->HolographicResult<A>{
        let result=trace.superpose(bound).map_err(|source|HolographicError::Algebra{operation:"trace superposition".into(),source})?;finite(&result)?;Ok(result)
    }
    fn binding(key:&A,value:&A)->HolographicResult<A>{key_inverse(key)?;finite(value)?;let b=key.bind(value);finite(&b)?;Ok(b)}
    fn next_count(&self,n:usize)->HolographicResult<usize>{self.item_count.checked_add(n).ok_or(HolographicError::CountOverflow)}
    /// Validate key, value and resulting trace before publishing any state.
    pub fn store(&mut self,key:&A,value:&A)->HolographicResult<()>{let n=self.next_count(1)?;let trace=Self::add(&self.trace,&Self::binding(key,value)?)?;if let Some(keys)=&mut self.stored_keys{keys.push(key.clone());}self.trace=trace;self.item_count=n;Ok(())}
    /// Store every pair in order or commit none of them.
    pub fn store_batch(&mut self,pairs:&[(A,A)])->HolographicResult<()>{let n=self.next_count(pairs.len())?;let mut trace=self.trace.clone();for(k,v)in pairs{trace=Self::add(&trace,&Self::binding(k,v)?)?;}if let Some(keys)=&mut self.stored_keys{keys.extend(pairs.iter().map(|(k,_)|k.clone()));}self.trace=trace;self.item_count=n;Ok(())}
    /// Concatenate compatible memories; an empty source never changes the destination.
    pub fn merge(&mut self,other:&Self)->HolographicResult<()>{if other.item_count==0{return Ok(());}if self.stored_keys.is_some()!=other.stored_keys.is_some() || self.config!=other.config{return Err(HolographicError::IncompatibleMemory);}let n=self.next_count(other.item_count)?;let trace=Self::add(&self.trace,&other.trace)?;if let(Some(dst),Some(src))=(&mut self.stored_keys,&other.stored_keys){dst.extend(src.iter().cloned());}self.trace=trace;self.item_count=n;Ok(())}
    /// Compare complete checked keys exactly, without similarity thresholds.
    pub fn membership(&self,key:&A)->HolographicResult<MembershipEvidence>{key_inverse(key)?;let Some(keys)=&self.stored_keys else{return Ok(MembershipEvidence::Unknown);};let query=key.full_coefficients();let indices:Vec<_>=keys.iter().enumerate().filter(|(_,k)|k.full_coefficients()==query).map(|(i,_)|i).collect();Ok(if indices.is_empty(){MembershipEvidence::Absent}else{MembershipEvidence::Present{indices}})}
    /// Closest tracked record, independently of membership. No score without keys.
    pub fn nearest_key(&self,key:&A)->HolographicResult<Option<NearestKey>>{key_inverse(key)?;let Some(keys)=&self.stored_keys else{return Ok(None);};let query=key.full_coefficients();let mut best:Option<NearestKey>=None;for(index,k)in keys.iter().enumerate(){let similarity=super::domain::cosine(&query,&k.full_coefficients())?;if best.as_ref().is_none_or(|b|similarity>b.similarity){best=Some(NearestKey{index,similarity});}}Ok(best)}
    /// Raw left unbinding. For an empty memory and valid key the result is zero.
    pub fn retrieve(&self,key:&A)->HolographicResult<RetrievalResult<A>>{let raw_value=key_inverse(key)?.bind(&self.trace);finite(&raw_value)?;Ok(RetrievalResult{raw_value})}
}
#[cfg(test)]
mod tests{
use super::*;use crate::Cl3;
#[test] fn u2_checked_count_before_commit(){let mut m=HolographicMemory::new(Default::default());m.item_count=usize::MAX;assert!(matches!(m.store(&Cl3::one(),&Cl3::one()),Err(HolographicError::CountOverflow)));assert_eq!(*m.trace(),Cl3::zero());}
#[test] fn test_memory_store_retrieve(){let mut m=HolographicMemory::new(Default::default());let k=Cl3::scalar(2.)+Cl3::unit_vector(0);let v=Cl3::unit_vector(1);m.store(&k,&v).unwrap();assert_eq!(m.retrieve(&k).unwrap().raw_value,v);assert!(m.retrieve(&Cl3::zero()).is_err());}
#[test] fn test_memory_clear(){let mut m=HolographicMemory::with_key_tracking(Default::default());m.store(&Cl3::one(),&Cl3::zero()).unwrap();m.clear();assert_eq!(m.item_count(),0);assert!(m.verify_consistency());}
#[test] fn test_memory_capacity_info(){let m=HolographicMemory::<Cl3>::new(Default::default());assert!(m.capacity_info().estimated_snr.is_infinite());assert!(!m.capacity_info().near_capacity);}
}
