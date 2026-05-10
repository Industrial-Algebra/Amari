use amari_rewrite::{Path, Rewritable};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Expr {
    Lit(i64),
    Add(Box<Expr>, Box<Expr>),
}

impl Rewritable for Expr {
    fn child_count(&self) -> usize {
        match self {
            Expr::Lit(_) => 0,
            Expr::Add(_, _) => 2,
        }
    }

    fn child(&self, index: usize) -> Option<&Self> {
        match self {
            Expr::Lit(_) => None,
            Expr::Add(left, right) => match index {
                0 => Some(left),
                1 => Some(right),
                _ => None,
            },
        }
    }

    fn replace_child(&self, index: usize, replacement: Self) -> amari_rewrite::RewriteResult<Self> {
        match (self, index) {
            (Expr::Add(_, right), 0) => Ok(Expr::Add(Box::new(replacement), right.clone())),
            (Expr::Add(left, _), 1) => Ok(Expr::Add(left.clone(), Box::new(replacement))),
            _ => Err(amari_rewrite::RewriteError::InvalidChildIndex { index }),
        }
    }
}

#[test]
fn positions_include_root_and_descendants() {
    let expr = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(2)));
    assert_eq!(expr.positions(), vec![Path::root(), Path::from([0]), Path::from([1])]);
}

#[test]
fn subterm_reads_by_path() {
    let expr = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(2)));
    assert_eq!(expr.subterm(&Path::from([1])), Some(&Expr::Lit(2)));
}

#[test]
fn replace_at_replaces_nested_subterm() {
    let expr = Expr::Add(
        Box::new(Expr::Lit(1)),
        Box::new(Expr::Add(Box::new(Expr::Lit(2)), Box::new(Expr::Lit(3)))),
    );

    let rewritten = expr.replace_at(&Path::from([1, 0]), Expr::Lit(20)).unwrap();

    assert_eq!(
        rewritten,
        Expr::Add(
            Box::new(Expr::Lit(1)),
            Box::new(Expr::Add(Box::new(Expr::Lit(20)), Box::new(Expr::Lit(3)))),
        )
    );
}
