// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_rewrite_macros::relation;

fn main() {
    let _ = relation!(exists X. f(X) <=> g(X));
}
