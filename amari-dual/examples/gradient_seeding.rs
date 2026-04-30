use amari_dual::MultiDualNumber;

fn main() {
    let vars = MultiDualNumber::variables(&[2.0, 3.0]);
    let x = vars[0].clone();
    let y = vars[1].clone();

    // f(x, y) = x² + xy + y²
    let result = x.clone() * x.clone() + x * y.clone() + y.clone() * y;

    println!("value: {}", result.get_value());
    println!("gradient: {:?}", result.get_gradient());
}
