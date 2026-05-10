//! Common imports for `amari-rewrite`.

pub use crate::{Path, RewriteError, RewriteResult, Rewritable};

pub use crate::ars::{Rule, Strategy, System};
pub use crate::trs::{Rule as TrsRule, Substitution, Term, TermSystem};
pub use crate::inverse::BackwardSearch;
pub use crate::synthesis::{anti_unify, anti_unify_all};
pub use crate::synthesis::{infer_rule, infer_rules};
