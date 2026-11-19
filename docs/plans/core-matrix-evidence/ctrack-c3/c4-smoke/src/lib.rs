//! C4 translation smoke (contract #350 S8): drive amari-core's
//! contracted surface through a carrier so translation of the real
//! obligations is exercised. Nothing here is PROVED.
use creusot_contracts::prelude::*;

#[requires(v.iter().all(|c| c.is_finite()))]
#[requires(v.len() == 8)]
pub fn smoke_ingress(v: &[f64]) -> u64 {
    match amari_core::verified::VerifiedMultivector::<f64, 3, 0, 0>::new(v.to_vec()) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}
