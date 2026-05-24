use amari_rewrite::{
    ars::{Rule, System},
    Rewritable, RewriteError, RewriteResult,
};

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

    fn replace_child(&self, index: usize, replacement: Self) -> RewriteResult<Self> {
        match (self, index) {
            (Expr::Add(_, right), 0) => Ok(Expr::Add(Box::new(replacement), right.clone())),
            (Expr::Add(left, _), 1) => Ok(Expr::Add(left.clone(), Box::new(replacement))),
            _ => Err(RewriteError::InvalidChildIndex { index }),
        }
    }
}

fn main() -> RewriteResult<()> {
    let system = System::new(vec![Rule::new("add-zero-left", |expr: &Expr| match expr {
        Expr::Add(left, right) if **left == Expr::Lit(0) => Some((**right).clone()),
        _ => None,
    })]);

    let expr = Expr::Add(
        Box::new(Expr::Lit(0)),
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(42)))),
    );

    let normalized = system.normalize_with_limit(&expr, 8)?;
    println!("{expr:?} => {normalized:?}");
    assert_eq!(normalized, Expr::Lit(42));
    Ok(())
}
