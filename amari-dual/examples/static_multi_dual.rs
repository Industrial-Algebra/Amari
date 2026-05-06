use amari_dual::StaticMultiDual;

fn main() {
    let [alpha, beta] = StaticMultiDual::<f64, 2>::variables([0.5, 1.25]);

    // Simple heuristic score: alpha^2 + 3 * beta
    let score = alpha * alpha + StaticMultiDual::constant(3.0) * beta;

    println!("score: {}", score.get_value());
    println!("gradient: {:?}", score.get_gradient());
}
