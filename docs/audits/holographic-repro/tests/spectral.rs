use amari_holographic::algebra::{
    logsumexp, BindingAlgebra as A, FHRRAlgebra as F, MAPAlgebra as M,
};
use num_complex::Complex64 as C;
const N: usize = 4;
type V = [C; N];
fn dft(a: V, inv: bool) -> V {
    std::array::from_fn(|k| {
        (0..N)
            .map(|n| {
                a[n] * C::from_polar(
                    1.,
                    (if inv { 1. } else { -1. }) * std::f64::consts::TAU * (n * k) as f64
                        / N as f64,
                )
            })
            .sum::<C>()
            / (if inv { N as f64 } else { 1. })
    })
}
fn pack(a: V) -> F<N> {
    F::new(a.map(|z| z.re), a.map(|z| z.im))
}
fn unpack(a: &F<N>) -> V {
    std::array::from_fn(|i| C::new(a.real(i).unwrap(), a.imag(i).unwrap()))
}
fn err(a: V, b: V) -> f64 {
    (0..N).map(|i| (a[i] - b[i]).norm()).fold(0., f64::max)
}
#[test]
fn oracle() {
    let x = [1., 2., 3., 4.].map(C::from);
    let y = [0., 1., 0., 0.].map(C::from);
    let k = pack(dft(y, false));
    let v = pack(dft(x, false));
    let b = k.bind(&v);
    let conv = std::array::from_fn(|n| (0..N).map(|m| y[m] * x[(n + N - m) % N]).sum());
    let e = err(dft(unpack(&b), true), conv);
    let r = err(dft(unpack(&k.unbind(&b).unwrap()), true), x);
    println!(
        "time=30 spectral={} bound={} conv={conv:?} maxerr={e:e} unbinderr={r:e} raw={:?}",
        v.norm_squared(),
        b.norm_squared(),
        pack(x).bind(&pack(y)).to_coefficients()
    );
    assert!(e < 1e-12 && r < 1e-12 && (v.norm_squared() - 120.).abs() < 1e-12);
}
#[test]
fn surface() {
    let f = F::new([2., 0.], [0., 3.]);
    let v = F::identity();
    println!(
        "fhrr energy {} -> {}, unbind {:?}, reciprocal product {:?}",
        v.norm_squared(),
        f.bind(&v).norm_squared(),
        f.unbind(&f.bind(&v)).unwrap(),
        f.component_inverse().unwrap().bind(&f)
    );
    let m = M::new([2., -3.]);
    println!(
        "map recovered {:?}; zero inverses {:?} {:?}",
        m.unbind(&m.bind(&M::identity())),
        F::<2>::zero().inverse(),
        M::<2>::zero().inverse()
    );
    for n in [1e200, f64::NAN] {
        println!(
            "normalized {n}: {:?} {:?}",
            F::new([n], [0.]).normalize(),
            M::new([n]).normalize()
        );
    }
    println!(
        "zero-dimensional panic F={} M={}",
        std::panic::catch_unwind(|| F::<0>::zero().permute(1)).is_err(),
        std::panic::catch_unwind(|| M::<0>::zero().permute(1)).is_err()
    );
    let a = M::new([2.]);
    let b = M::new([0.]);
    println!(
        "MAP bundle {:?} all {:?} beta0 {:?}",
        a.bundle(&b, 1.),
        M::bundle_all(&[a.clone(), b.clone()], 1.),
        a.bundle(&a, 0.)
    );
    println!(
        "lse {} {}, bad vec {:?}",
        logsumexp(0., 0., 0.),
        logsumexp(0., 0., 1e-6),
        F::<2>::from_vecs(vec![0.; 3], vec![0.; 2])
    );
}
#[test]
fn dense_oracle() {
    let x = [
        C::new(1., 2.),
        C::new(-2., 1.),
        C::new(0., 3.),
        C::new(4., -1.),
    ];
    let y = [2., 0., 1., 0.].map(C::from);
    let v = pack(dft(x, false));
    let k = pack(dft(y, false));
    let b = k.bind(&v);
    let conv = std::array::from_fn(|n| (0..N).map(|m| y[m] * x[(n + N - m) % N]).sum());
    let e = err(dft(unpack(&b), true), conv);
    let r = err(
        dft(unpack(&k.component_inverse().unwrap().bind(&b)), true),
        x,
    );
    let corr = err(dft(unpack(&k.unbind(&b).unwrap()), true), x);
    println!(
        "dense conv={e:e} inverse={r:e} adjoint error={corr} E={} -> {}",
        v.norm_squared(),
        b.norm_squared()
    );
    assert!(e < 1e-12 && r < 1e-12 && corr > 1.);
}
#[test]
fn shift_bundle() {
    let k = F::<4>::identity();
    println!(
        "permuted spectrum {:?}; IDFT={:?}",
        k.permute(1),
        dft(unpack(&k.permute(1)), true)
    );
    let a = F::new([1.], [0.]);
    let b = a.scale(3.);
    println!(
        "fhrr hard={:?} beta0={:?} all(0,0,4)={:?}",
        a.bundle(&b, f64::INFINITY),
        a.bundle(&b, 0.),
        F::bundle_all(&[F::zero(), F::zero(), a.scale(4.)], 0.)
    );
    let m = M::new([-1.]);
    let p = M::new([1.]);
    println!(
        "MAP distributivity: {:?} vs {:?}",
        m.bind(&p.bundle(&m, f64::INFINITY).unwrap()),
        m.bind(&p).bundle(&m.bind(&m), f64::INFINITY)
    );
    let n = k.normalize().unwrap();
    println!(
        "normalized key: norm={}, mag={}, recovered {:?}",
        n.norm(),
        n.magnitude(0).unwrap(),
        n.unbind(&n.bind(&k))
    );
}
