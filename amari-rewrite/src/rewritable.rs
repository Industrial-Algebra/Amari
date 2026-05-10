use alloc::vec::Vec;
use core::fmt::Debug;

use crate::{RewriteError, RewriteResult};

/// Path from a root term to one of its subterms.
///
/// The empty path is the root. Each component is a child index at the current
/// node.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path(Vec<usize>);

impl Path {
    /// Return the root path.
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// Borrow the path components.
    pub fn as_slice(&self) -> &[usize] {
        &self.0
    }

    /// Return a path extended with one child index.
    pub fn child(&self, index: usize) -> Self {
        let mut next = self.0.clone();
        next.push(index);
        Self(next)
    }

    /// Whether this path identifies the root term.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl<const N: usize> From<[usize; N]> for Path {
    fn from(value: [usize; N]) -> Self {
        Self(value.into())
    }
}

impl From<Vec<usize>> for Path {
    fn from(value: Vec<usize>) -> Self {
        Self(value)
    }
}

/// A recursive value that can be traversed and rebuilt at subterm paths.
///
/// Implement this trait for user-owned ASTs or state trees to make them usable
/// by the abstract rewriting-system layer.
pub trait Rewritable: Clone + PartialEq + Debug {
    /// Number of immediate children.
    fn child_count(&self) -> usize;

    /// Return an immediate child by index.
    fn child(&self, index: usize) -> Option<&Self>;

    /// Return a new value with one immediate child replaced.
    fn replace_child(&self, index: usize, replacement: Self) -> RewriteResult<Self>;

    /// Borrow a subterm by path.
    fn subterm(&self, path: &Path) -> Option<&Self> {
        let mut current = self;
        for &index in path.as_slice() {
            current = current.child(index)?;
        }
        Some(current)
    }

    /// Return a new value with the subterm at `path` replaced.
    fn replace_at(&self, path: &Path, replacement: Self) -> RewriteResult<Self> {
        self.replace_at_slice(path.as_slice(), replacement)
    }

    /// Return all valid positions in preorder, including the root.
    fn positions(&self) -> Vec<Path> {
        fn walk<T: Rewritable>(term: &T, path: Path, out: &mut Vec<Path>) {
            out.push(path.clone());
            for index in 0..term.child_count() {
                if let Some(child) = term.child(index) {
                    walk(child, path.child(index), out);
                }
            }
        }

        let mut out = Vec::new();
        walk(self, Path::root(), &mut out);
        out
    }

    fn replace_at_slice(&self, path: &[usize], replacement: Self) -> RewriteResult<Self> {
        match path.split_first() {
            None => Ok(replacement),
            Some((&index, rest)) => {
                let child = self
                    .child(index)
                    .ok_or(RewriteError::InvalidChildIndex { index })?;
                let replaced_child = child.replace_at_slice(rest, replacement)?;
                self.replace_child(index, replaced_child)
            }
        }
    }
}
