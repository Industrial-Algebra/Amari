use amari_dual::{BranchPolicy, DualNumber};

fn main() {
    let left = DualNumber::new(1.0, 2.0);
    let right = DualNumber::new(1.0, 6.0);

    let left_biased = left.max(right);
    let right_biased = left.max_by_policy(right, BranchPolicy::Right);
    let averaged = left.max_by_policy(right, BranchPolicy::Average);

    println!("left-biased tie derivative: {}", left_biased.derivative());
    println!("right-biased tie derivative: {}", right_biased.derivative());
    println!("averaged tie derivative: {}", averaged.derivative());
}
