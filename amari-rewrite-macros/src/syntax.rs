// SPDX-License-Identifier: MIT OR Apache-2.0

//! Checked `term!` and `rule!` construction macros.
//!
//! Grammar:
//!
//! ```text
//! term     := IDENT | STRING | IDENT "(" args ")" | STRING "(" args ")"
//! rule     := term "=>" term
//! ```
//!
//! Lowercase-leading identifiers and string literals are symbols
//! (`term!(f)` == `term!("f")`); uppercase-leading identifiers are
//! variables. A bare symbol (or one with empty parentheses) is a
//! constant. Variables never take arguments.
//!
//! `rule!` expands to fully qualified
//! `amari_rewrite::trs::Rule::new(lhs, rhs)` — checked construction
//! (right-hand-side variables must occur on the left), never
//! `new_unchecked`. All diagnostics use single-token spans so trybuild
//! fixtures render identically across rustc versions.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parenthesized, Error, Ident, LitStr, Token};

use crate::paths::rewrite_path;

/// A parsed term: the shared front end of `term!`, `rule!`, and
/// `relation!`. Variable occurrences keep their spans so relation
/// diagnostics point at the exact token that is wrong.
pub(crate) enum TermSyntax {
    Symbol { name: String, args: Vec<TermSyntax> },
    Variable { name: String, span: Span },
}

impl TermSyntax {
    pub(crate) fn expand(&self, rewrite: &TokenStream) -> TokenStream {
        match self {
            TermSyntax::Variable { name, .. } => {
                quote!(#rewrite::trs::Term::var(#name))
            }
            TermSyntax::Symbol { name, args } if args.is_empty() => {
                quote!(#rewrite::trs::Term::constant(#name))
            }
            TermSyntax::Symbol { name, args } => {
                let expanded = args.iter().map(|arg| arg.expand(rewrite));
                quote!(#rewrite::trs::Term::sym(#name, [#(#expanded),*]))
            }
        }
    }

    /// Collect variable occurrences in structural order (each
    /// duplicate is kept: erasure checks care about names, not
    /// counts, but spans point at every occurrence).
    pub(crate) fn collect_variables(&self, out: &mut Vec<(String, Span)>) {
        match self {
            TermSyntax::Variable { name, span } => out.push((name.clone(), *span)),
            TermSyntax::Symbol { args, .. } => {
                for arg in args {
                    arg.collect_variables(out);
                }
            }
        }
    }
}

fn is_variable_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// The `term!` variable naming rule (uppercase-leading), shared with
/// `relation!`'s existential-marker validation.
pub(crate) fn is_variable(name: &str) -> bool {
    is_variable_name(name)
}

impl Parse for TermSyntax {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        let (name, span) = if lookahead.peek(Ident) {
            let ident: Ident = input.parse()?;
            (ident.to_string(), ident.span())
        } else if lookahead.peek(LitStr) {
            let literal: LitStr = input.parse()?;
            (literal.value(), literal.span())
        } else {
            return Err(lookahead.error());
        };

        if !input.peek(syn::token::Paren) {
            return Ok(if is_variable_name(&name) {
                TermSyntax::Variable { name, span }
            } else {
                TermSyntax::Symbol {
                    name,
                    args: Vec::new(),
                }
            });
        }

        if is_variable_name(&name) {
            return Err(Error::new(span, "variables cannot have arguments"));
        }
        let content;
        parenthesized!(content in input);
        let args = content
            .parse_terminated(TermSyntax::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(TermSyntax::Symbol { name, args })
    }
}

struct RuleSyntax {
    lhs: TermSyntax,
    rhs: TermSyntax,
}

impl Parse for RuleSyntax {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lhs: TermSyntax = input.parse()?;
        input.parse::<Token![=>]>()?;
        let rhs: TermSyntax = input.parse()?;
        if !input.is_empty() {
            return Err(Error::new(
                input.span(),
                "unexpected tokens after rule right-hand side",
            ));
        }
        Ok(RuleSyntax { lhs, rhs })
    }
}

pub fn expand_term(input: TokenStream) -> TokenStream {
    let parsed = syn::parse2::<TermSyntax>(input);
    match parsed {
        Ok(term) => {
            let rewrite = rewrite_path();
            term.expand(&rewrite)
        }
        Err(error) => error.into_compile_error(),
    }
}

pub fn expand_rule(input: TokenStream) -> TokenStream {
    let parsed = syn::parse2::<RuleSyntax>(input);
    match parsed {
        Ok(rule) => {
            let rewrite = rewrite_path();
            let lhs = rule.lhs.expand(&rewrite);
            let rhs = rule.rhs.expand(&rewrite);
            quote! {{
                let __rewritable_lhs = #lhs;
                let __rewritable_rhs = #rhs;
                #rewrite::trs::Rule::new(__rewritable_lhs, __rewritable_rhs)
            }}
        }
        Err(error) => error.into_compile_error(),
    }
}
