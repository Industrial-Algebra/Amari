// SPDX-License-Identifier: MIT OR Apache-2.0

//! Checked relational declarations: `relation!(lhs <=> rhs)`.
//!
//! Grammar:
//!
//! ```text
//! relation   := [ "exists" VAR { "," VAR } "." ] term "<=>" term
//!               [ "where" constraint { "," constraint } ]
//! constraint := term ( "==" | "!=" ) term
//! ```
//!
//! Expansion is always
//! `RewriteResult<(trs::Rule, relation::BackwardClause,
//! Option<trs::Rule>, relation::ConstraintSet)>`: the checked forward
//! rule, its compiled backward contract, the checked reverse rule
//! exactly when the reverse direction erases nothing, and the declared
//! constraints in declaration order. Every element is produced by a
//! checked constructor (`trs::Rule::new`, `relation::BackwardClause::
//! compile`); no closure, no `Rule::new_unchecked`, no runtime
//! registration, and no hidden state. The output is a pure function of
//! the input tokens, and the generated code only names `::core` and the
//! resolved `amari-rewrite` crate, so it works in `no_std` consumers.
//!
//! When the reverse direction would invent variables (`lhs` binds a
//! variable that `rhs` does not), no reverse rule is fabricated: the
//! declaration yields the compiled clause instead, whose
//! `erased_variables()` are exactly the residual schema names a
//! `reversible::RewriteResidual` binding records. A forward direction
//! that would invent variables is a compile error, because a checked
//! forward rule must bind every right-hand-side variable.
//!
//! `exists` markers are an optional assertion: when present they must
//! name exactly the variables the reverse direction erases, so a
//! declaration cannot silently depend on erasure it did not intend.
//! `exists` is only a marker when an existential variable follows it, so
//! the symbol `exists` stays usable as a term.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Error, Ident, Token};

use crate::paths::rewrite_path;
use crate::syntax::{is_variable, TermSyntax};

/// One declared constraint: `term ("==" | "!=") term`.
struct ConstraintSyntax {
    left: TermSyntax,
    op: ConstraintOp,
    right: TermSyntax,
}

enum ConstraintOp {
    Equal,
    NotEqual,
}

/// A parsed relation declaration.
pub(crate) struct RelationSyntax {
    existentials: Vec<(String, Span)>,
    lhs: TermSyntax,
    rhs: TermSyntax,
    constraints: Vec<ConstraintSyntax>,
}

fn error_at(span: Span, message: impl core::fmt::Display) -> Error {
    Error::new(span, message)
}

/// Accumulate diagnostics so one declaration reports every offending
/// token with its own span instead of only the first.
fn push(slot: &mut Option<Error>, error: Error) {
    match slot {
        Some(existing) => existing.combine(error),
        None => *slot = Some(error),
    }
}

fn variables(term: &TermSyntax) -> Vec<(String, Span)> {
    let mut collected = Vec::new();
    term.collect_variables(&mut collected);
    collected
}

fn contains(variables: &[(String, Span)], name: &str) -> bool {
    variables.iter().any(|(candidate, _)| candidate == name)
}

impl Parse for RelationSyntax {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let existentials = parse_existentials(input)?;

        let lhs: TermSyntax = input.parse()?;
        let arrow = input.span();
        if input.peek(Token![=>]) {
            return Err(error_at(
                arrow,
                "`relation!` requires the bidirectional `<=>`: a lone `=>` is \
                 ambiguous between a one-directional TRS rewrite rule and an \
                 ARS rewrite step; use `rule!(lhs => rhs)` for one direction \
                 or `relation!(lhs <=> rhs)` for both",
            ));
        }
        // `<=>` arrives as the joint punct run `<=` `>`: parse both halves
        // to decide, rather than peeking across the punct run.
        let fork = input.fork();
        if fork.parse::<Token![<=]>().is_ok() && fork.parse::<Token![>]>().is_ok() {
            input.parse::<Token![<=]>()?;
            input.parse::<Token![>]>()?;
        } else if input.peek(Token![<=]) {
            return Err(error_at(
                arrow,
                "`relation!` requires the bidirectional `<=>`: a lone `<=` \
                 is ambiguous between a reversed relation direction and a \
                 backward ARS step; write `relation!(lhs <=> rhs)`",
            ));
        } else {
            return Err(error_at(arrow, "expected `<=>` in a relation declaration"));
        }

        let rhs: TermSyntax = input.parse()?;
        let constraints = parse_constraints(input)?;

        if !input.is_empty() {
            return Err(error_at(
                input.span(),
                "unexpected tokens after the relation right-hand side",
            ));
        }

        Ok(RelationSyntax {
            existentials,
            lhs,
            rhs,
            constraints,
        })
    }
}

/// Parse the optional `exists V {, V} .` prefix.
fn parse_existentials(input: ParseStream) -> syn::Result<Vec<(String, Span)>> {
    let mut existentials = Vec::new();
    if !input.peek(Ident) {
        return Ok(existentials);
    }
    // `exists` is a marker only when an existential variable follows:
    // another identifier or a comma. Otherwise it stays a symbol name.
    let fork = input.fork();
    let keyword: Ident = fork.parse()?;
    if keyword != "exists" || !(fork.peek(Ident) || fork.peek(Token![,])) {
        return Ok(existentials);
    }
    input.parse::<Ident>()?;

    loop {
        if !input.peek(Ident) {
            return Err(error_at(
                input.span(),
                "expected an existential variable name after `exists`",
            ));
        }
        let variable: Ident = input.parse()?;
        let name = variable.to_string();
        if !is_variable(&name) {
            return Err(error_at(
                variable.span(),
                "existential variables must be uppercase-leading identifiers, \
                 matching `term!` variable syntax",
            ));
        }
        existentials.push((name, variable.span()));
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            continue;
        }
        break;
    }

    if !input.peek(Token![.]) {
        return Err(error_at(
            input.span(),
            "expected `.` after the existential variables",
        ));
    }
    input.parse::<Token![.]>()?;
    Ok(existentials)
}

/// Parse the optional `where constraint {, constraint}` suffix.
fn parse_constraints(input: ParseStream) -> syn::Result<Vec<ConstraintSyntax>> {
    let mut constraints = Vec::new();
    if !input.peek(Token![where]) {
        return Ok(constraints);
    }
    let where_span = input.span();
    input.parse::<Token![where]>()?;
    if input.is_empty() {
        return Err(error_at(
            where_span,
            "expected at least one relation constraint after `where`",
        ));
    }

    loop {
        let left_span = input.span();
        let left: TermSyntax = input.parse()?;
        let operator_span = input.span();
        let op = if input.peek(Token![==]) {
            input.parse::<Token![==]>()?;
            ConstraintOp::Equal
        } else if input.peek(Token![!=]) {
            input.parse::<Token![!=]>()?;
            ConstraintOp::NotEqual
        } else {
            return Err(error_at(
                left_span,
                "expected `==` or `!=` after a relation constraint term",
            ));
        };
        if input.is_empty() || input.peek(Token![,]) {
            return Err(error_at(
                operator_span,
                "expected a term on the right of the relation constraint operator",
            ));
        }
        let right: TermSyntax = input.parse()?;
        constraints.push(ConstraintSyntax { left, op, right });

        if input.peek(Token![,]) {
            let comma_span = input.span();
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                return Err(error_at(
                    comma_span,
                    "expected a relation constraint after `,`",
                ));
            }
            continue;
        }
        break;
    }
    Ok(constraints)
}

/// Variables the reverse direction (`rhs => lhs`) would invent, in
/// first-occurrence order of the left-hand side.
fn erased_variables(lhs: &[(String, Span)], rhs: &[(String, Span)]) -> Vec<(String, Span)> {
    lhs.iter()
        .filter(|(name, _)| !contains(rhs, name))
        .cloned()
        .collect()
}

impl RelationSyntax {
    /// Static checks: every diagnostic here is a declaration the
    /// checked relation model cannot represent, never a runtime
    /// judgment the constructors already make.
    fn validate(&self) -> syn::Result<()> {
        let lhs = variables(&self.lhs);
        let rhs = variables(&self.rhs);

        // A checked forward rule must bind every RHS variable.
        let mut errors: Option<Error> = None;
        for (name, span) in &rhs {
            if !contains(&lhs, name) {
                push(
                    &mut errors,
                    error_at(
                        *span,
                        format!(
                            "the forward direction `lhs => rhs` introduces variable \
                             `{name}`, which the left-hand side does not bind; a checked \
                             forward rule cannot invent variables"
                        ),
                    ),
                );
            }
        }
        if let Some(errors) = errors {
            return Err(errors);
        }

        // Existential markers must name variables bound by exactly one
        // side.
        for (name, span) in &self.existentials {
            let on_left = contains(&lhs, name);
            let on_right = contains(&rhs, name);
            if on_left && on_right {
                push(
                    &mut errors,
                    error_at(
                        *span,
                        format!(
                            "existential variable `{name}` occurs on both sides of the \
                             relation; an existential variable must be bound by exactly \
                             one side"
                        ),
                    ),
                );
            } else if !on_left && !on_right {
                push(
                    &mut errors,
                    error_at(
                        *span,
                        format!(
                            "existential variable `{name}` does not occur on either side \
                             of the relation"
                        ),
                    ),
                );
            }
        }
        if let Some(errors) = errors {
            return Err(errors);
        }

        // When markers are declared they must cover the erasure
        // exactly; otherwise the declaration would depend on an
        // erasure the author did not state.
        let erased = erased_variables(&lhs, &rhs);
        if !self.existentials.is_empty() {
            for (name, span) in &erased {
                if !contains(&self.existentials, name) {
                    push(
                        &mut errors,
                        error_at(
                            *span,
                            format!(
                                "the reverse direction erases variable `{name}`, but the \
                                 declaration does not mark it `exists`; mark it `exists` \
                                 or omit the existential marker entirely"
                            ),
                        ),
                    );
                }
            }
        }

        // Constraint variables must be bound by the relation itself,
        // or the constraint set would mention terms the clause cannot
        // reach.
        for constraint in &self.constraints {
            for term in [&constraint.left, &constraint.right] {
                for (name, span) in variables(term) {
                    if !contains(&lhs, &name) && !contains(&rhs, &name) {
                        push(
                            &mut errors,
                            error_at(
                                span,
                                format!(
                                    "relation constraint variable `{name}` does not occur \
                                     in the relation"
                                ),
                            ),
                        );
                    }
                }
            }
        }

        match errors {
            Some(errors) => Err(errors),
            None => Ok(()),
        }
    }

    /// Expand to fully qualified checked constructors.
    fn expand(self) -> TokenStream {
        let rewrite = rewrite_path();
        let lhs = self.lhs.expand(&rewrite);
        let rhs = self.rhs.expand(&rewrite);

        let inserts: Vec<TokenStream> = self
            .constraints
            .iter()
            .map(|constraint| {
                let left = constraint.left.expand(&rewrite);
                let right = constraint.right.expand(&rewrite);
                let constructor = match constraint.op {
                    ConstraintOp::Equal => quote!(#rewrite::relation::TermConstraint::Equal),
                    ConstraintOp::NotEqual => quote!(#rewrite::relation::TermConstraint::NotEqual),
                };
                quote!(__amari_relation_constraints.insert(#constructor(#left, #right));)
            })
            .collect();
        // Only bind the set mutably when something is inserted, so the
        // expansion never trips `unused_mut` in user code.
        let mutability = if inserts.is_empty() {
            quote!()
        } else {
            quote!(mut)
        };

        let reverse_erases =
            !erased_variables(&variables(&self.lhs), &variables(&self.rhs)).is_empty();
        // The reversible form needs both terms twice; the erasing form
        // consumes each exactly once.
        let (forward_lhs, forward_rhs, tail) = if reverse_erases {
            (
                quote!(__amari_relation_lhs),
                quote!(__amari_relation_rhs),
                quote! {
                    ::core::result::Result::Ok((
                        __amari_relation_forward,
                        __amari_relation_backward,
                        ::core::option::Option::None::<#rewrite::trs::Rule>,
                        __amari_relation_constraints,
                    ))
                },
            )
        } else {
            (
                quote!(__amari_relation_lhs.clone()),
                quote!(__amari_relation_rhs.clone()),
                quote! {
                    match #rewrite::trs::Rule::new(__amari_relation_rhs, __amari_relation_lhs) {
                        ::core::result::Result::Ok(__amari_relation_reverse) =>
                            ::core::result::Result::Ok((
                                __amari_relation_forward,
                                __amari_relation_backward,
                                ::core::option::Option::Some(__amari_relation_reverse),
                                __amari_relation_constraints,
                            )),
                        ::core::result::Result::Err(__amari_relation_error) =>
                            ::core::result::Result::Err(__amari_relation_error),
                    }
                },
            )
        };

        quote! {{
            let __amari_relation_lhs = #lhs;
            let __amari_relation_rhs = #rhs;
            let #mutability __amari_relation_constraints =
                #rewrite::relation::ConstraintSet::new();
            #(#inserts)*
            match #rewrite::trs::Rule::new(#forward_lhs, #forward_rhs) {
                ::core::result::Result::Ok(__amari_relation_forward) => {
                    match #rewrite::relation::BackwardClause::compile(&__amari_relation_forward) {
                        ::core::result::Result::Ok(__amari_relation_backward) => #tail,
                        ::core::result::Result::Err(__amari_relation_error) =>
                            ::core::result::Result::Err(__amari_relation_error),
                    }
                }
                ::core::result::Result::Err(__amari_relation_error) =>
                    ::core::result::Result::Err(__amari_relation_error),
            }
        }}
    }
}

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match syn::parse2::<RelationSyntax>(input) {
        Ok(relation) => match relation.validate() {
            Ok(()) => relation.expand(),
            Err(error) => error.into_compile_error(),
        },
        Err(error) => error.into_compile_error(),
    }
}
