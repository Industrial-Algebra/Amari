// SPDX-License-Identifier: MIT OR Apache-2.0

//! Checked procedural macros for `amari-rewrite`.
//!
//! This crate is the only procedural-macro surface for the rewrite engine.
//! Algorithms stay in `amari-rewrite`; this crate contains parsing and code
//! generation only, so the macro crate never depends on the library crate
//! and always publishes before it.
//!
//! - `#[derive(Rewritable)]` — generates `Rewritable` implementations
//!   for structs and enums with explicit `#[rewritable(child)]` fields.
//! - `term!(f(a, X))` — checked `trs::Term` construction (Task 5).
//! - `rule!(lhs => rhs)` — checked `trs::Rule` construction (Task 5).
//! - `relation!(lhs <=> rhs)` — checked relational construction
//!   (Cohort 7).
//!
//! Every macro expands to fully qualified checked constructors in
//! `amari-rewrite`; nothing here registers runtime state, and the
//! expansion output is a pure function of the input tokens.

use proc_macro::TokenStream;

mod paths;
mod relation;
mod rewritable_derive;
mod syntax;

/// Derive `Rewritable` for an expression tree.
///
/// Children are exactly the fields marked `#[rewritable(child)]`, in
/// declaration order; recursion is never inferred. A child field's type
/// must deref to `Self` and rebuild from `Self` (`Box<Self>`,
/// `Rc<Self>`, `Arc<Self>`); unmarked fields are cloned through
/// replacement and must be `Clone`. Unions, generics, collections as
/// children, and malformed attributes are compile errors at precise
/// spans.
#[proc_macro_derive(Rewritable, attributes(rewritable))]
pub fn derive_rewritable(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    rewritable_derive::expand(input).into()
}

/// Checked `trs::Term` construction: `term!(add(zero, X))`.
///
/// Lowercase-leading identifiers and string literals are symbols
/// (`term!(f)` == `term!("f")`); uppercase-leading identifiers are
/// variables. A bare symbol (or empty parentheses) is a constant;
/// variables never take arguments. Malformed terms are compile errors.
#[proc_macro]
pub fn term(input: TokenStream) -> TokenStream {
    syntax::expand_term(input.into()).into()
}

/// Checked `trs::Rule` construction: `rule!(add(zero, X) => X)`.
///
/// Expands to fully qualified `amari_rewrite::trs::Rule::new` — checked
/// construction returning `RewriteResult<trs::Rule>` (right-hand-side
/// variables must occur on the left), never the ARS `Rule` type and
/// never unchecked construction.
#[proc_macro]
pub fn rule(input: TokenStream) -> TokenStream {
    syntax::expand_rule(input.into()).into()
}

/// Checked relational construction: `relation!(add(zero, X) <=> X)`.
///
/// Grammar:
///
/// ```text
/// relation   := [ "exists" VAR { "," VAR } "." ] term "<=>" term
///               [ "where" constraint { "," constraint } ]
/// constraint := term ( "==" | "!=" ) term
/// ```
///
/// The declaration expands to
/// `RewriteResult<(trs::Rule, relation::BackwardClause, Option<trs::Rule>,
/// relation::ConstraintSet)>`, built only from checked constructors:
///
/// - the forward rule, via `trs::Rule::new` (an RHS variable the LHS does
///   not bind is a compile error here, not a runtime error);
/// - its compiled backward contract, via
///   `relation::BackwardClause::compile`;
/// - the reverse rule `Some(..)` exactly when the reverse direction is
///   itself a checked rule, and `None` when the reverse direction
///   erases variables — then `BackwardClause::erased_variables()` names
///   the residual schema the reverse direction needs. No
///   `Rule::new_unchecked` and no closure is ever generated;
/// - the declared constraints, in declaration order, ready for
///   `relation::ConstraintSet::normalize`.
///
/// `exists` markers are optional and assert that the named variables
/// are the ones the reverse direction erases (they must occur on
/// exactly one side and the marker set must equal the erased set).
/// `exists` counts as a marker only when an existential variable
/// follows, so a symbol named `exists` still parses as a term.
///
/// A lone `=>` or `<=` is rejected: it is ambiguous between a
/// one-directional TRS rule, an ARS step, and a reversed relation
/// direction. Malformed constraints, unmarked erasure, and forward
/// variable introduction are compile errors at the offending token.
///
/// The generated code names only `::core` and the resolved
/// `amari-rewrite` crate path (renamed dependencies included), so the
/// expansion is usable from `no_std` consumers.
#[proc_macro]
pub fn relation(input: TokenStream) -> TokenStream {
    relation::expand(input.into()).into()
}
