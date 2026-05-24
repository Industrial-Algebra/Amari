//! Pattern matching for first-order terms.

use super::{Substitution, Term};

/// Match a pattern against a term, returning a consistent substitution.
///
/// Variables in `pattern` may bind to arbitrary subterms in `term`. Repeated
/// occurrences of a variable must bind to the same term.
pub fn match_pattern(pattern: &Term, term: &Term) -> Option<Substitution> {
    let mut subst = Substitution::new();
    match_into(pattern, term, &mut subst).then_some(subst)
}

fn match_into(pattern: &Term, term: &Term, subst: &mut Substitution) -> bool {
    match (pattern, term) {
        (Term::Var(var), _) => match subst.get_var(var) {
            Some(existing) => existing == term,
            None => {
                subst.insert(var.clone(), term.clone());
                true
            }
        },
        (Term::Sym(pattern_symbol, pattern_args), Term::Sym(term_symbol, term_args)) => {
            pattern_symbol == term_symbol
                && pattern_args.len() == term_args.len()
                && pattern_args
                    .iter()
                    .zip(term_args)
                    .all(|(pattern_arg, term_arg)| match_into(pattern_arg, term_arg, subst))
        }
        _ => false,
    }
}
