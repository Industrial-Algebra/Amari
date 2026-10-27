//! C3 conformance fixtures (contract #350 S6). Both directions in one
//! crate: `sum_to` carries a TRUE postcondition (must prove);
//! `wrong_successor` carries a DELIBERATELY FALSE one (must stay
//! unproved and fail the gate). Outcome parsing lives in c3-gate.sh.
use creusot_contracts::prelude::*;

#[requires(n@ <= 1000)]
#[ensures(result@ == n@ * (n@ + 1) / 2)]
pub fn sum_to(n: u64) -> u64 {
    let mut acc: u64 = 0;
    let mut i: u64 = 0;
    #[invariant(i@ <= n@ + 1)]
    #[invariant(acc@ == (i@ - 1) * i@ / 2)]
    while i <= n {
        acc += i;
        i += 1;
    }
    acc
}

// Negative fixture: deliberately FALSE postcondition. The gate must
// report this UNPROVED — a green exit here would invalidate the gate.
#[ensures(result@ == n@)]
pub fn wrong_successor(n: u64) -> u64 {
    n + 1
}
