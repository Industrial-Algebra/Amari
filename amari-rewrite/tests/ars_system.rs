use amari_rewrite::{
    ars::{Rule, System},
    Rewritable,
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

    fn replace_child(&self, index: usize, replacement: Self) -> amari_rewrite::RewriteResult<Self> {
        match (self, index) {
            (Expr::Add(_, right), 0) => Ok(Expr::Add(Box::new(replacement), right.clone())),
            (Expr::Add(left, _), 1) => Ok(Expr::Add(left.clone(), Box::new(replacement))),
            _ => Err(amari_rewrite::RewriteError::InvalidChildIndex { index }),
        }
    }
}

#[test]
fn normalize_applies_rule_until_fixed_point() {
    let system = System::new(vec![Rule::new("add-zero-left", |expr: &Expr| match expr {
        Expr::Add(left, right) if **left == Expr::Lit(0) => Some((**right).clone()),
        _ => None,
    })]);

    let expr = Expr::Add(
        Box::new(Expr::Lit(0)),
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(5)))),
    );

    assert_eq!(system.normalize_with_limit(&expr, 8).unwrap(), Expr::Lit(5));
}

#[test]
fn all_successors_returns_every_one_step_rewrite() {
    let system = System::new(vec![Rule::new("add-zero-left", |expr: &Expr| match expr {
        Expr::Add(left, right) if **left == Expr::Lit(0) => Some((**right).clone()),
        _ => None,
    })]);

    let expr = Expr::Add(
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(1)))),
        Box::new(Expr::Add(Box::new(Expr::Lit(0)), Box::new(Expr::Lit(2)))),
    );

    let successors = system.successors(&expr).unwrap();
    assert_eq!(successors.len(), 2);
}
