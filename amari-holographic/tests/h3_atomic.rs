use amari_holographic::{
    memory::MemoryElement, AlgebraError, AlgebraResult, BindingAlgebra, Cl3, FHRRAlgebra,
    HolographicError, HolographicMemory, MAPAlgebra, MembershipEvidence,
};

// Delegate scalar arithmetic to MAP. Only adding the marked bound value (13)
// to a nonempty fixture trace fails; the same value can seed a merge source.
#[derive(Clone, Debug)]
struct Fault(MAPAlgebra<1>);
impl Fault {
    fn scalar(x: f64) -> Self {
        Self(MAPAlgebra::new([x]))
    }
    fn x(&self) -> f64 {
        self.0.components()[0]
    }
}
impl MemoryElement for Fault {
    fn full_coefficients(&self) -> Vec<f64> {
        self.to_coefficients()
    }
    fn admissible_key(&self) -> bool {
        self.x().abs() == 1.
    }
}
impl BindingAlgebra for Fault {
    fn dimension(&self) -> usize {
        1
    }
    fn identity() -> Self {
        Self::scalar(1.)
    }
    fn zero() -> Self {
        Self::scalar(0.)
    }
    fn bind(&self, b: &Self) -> Self {
        Self(self.0.bind(&b.0))
    }
    fn inverse(&self) -> AlgebraResult<Self> {
        self.0.inverse().map(Self)
    }
    fn bundle(&self, b: &Self, beta: f64) -> AlgebraResult<Self> {
        self.0.bundle(&b.0, beta).map(Self)
    }
    fn superpose(&self, b: &Self) -> AlgebraResult<Self> {
        if self.x() != 0. && b.x() == 13. {
            return Err(AlgebraError::NumericalError {
                operation: "marked sum".into(),
                details: format!("{} + {}", self.x(), b.x()),
            });
        }
        self.0.superpose(&b.0).map(Self)
    }
    fn similarity(&self, b: &Self) -> f64 {
        self.0.similarity(&b.0)
    }
    fn norm(&self) -> f64 {
        self.0.norm()
    }
    fn normalize(&self) -> AlgebraResult<Self> {
        self.0.normalize().map(Self)
    }
    fn permute(&self, shift: i32) -> Self {
        Self(self.0.permute(shift))
    }
    fn get(&self, i: usize) -> AlgebraResult<f64> {
        self.0.get(i)
    }
    fn set(&mut self, i: usize, v: f64) -> AlgebraResult<()> {
        self.0.set(i, v)
    }
    fn from_coefficients(v: &[f64]) -> AlgebraResult<Self> {
        MAPAlgebra::from_coefficients(v).map(Self)
    }
    fn to_coefficients(&self) -> Vec<f64> {
        self.0.to_coefficients()
    }
    fn algebra_name() -> &'static str {
        "fault-injected scalar"
    }
}
fn seeded() -> HolographicMemory<Fault> {
    let mut m = HolographicMemory::with_key_tracking(Default::default());
    m.store(&Fault::identity(), &Fault::scalar(3.)).unwrap();
    m
}
// A single coefficient's bits and membership indices for the entire key domain
// capture the raw trace and every tracked record in order, including duplicates.
fn snapshot(m: &HolographicMemory<Fault>) -> (u64, usize, [MembershipEvidence; 2]) {
    (
        m.trace().x().to_bits(),
        m.item_count(),
        [1., -1.].map(|k| m.membership(&Fault::scalar(k)).unwrap()),
    )
}
fn assert_cause(err: HolographicError, sum: &str) {
    match err {
        HolographicError::Algebra {
            operation,
            source:
                AlgebraError::NumericalError {
                    operation: inner,
                    details,
                },
        } => {
            assert_eq!(operation, "trace superposition");
            assert_eq!(inner, "marked sum");
            assert_eq!(details, sum);
        }
        other => panic!("lost injected arithmetic cause: {other:?}"),
    }
}
#[test]
fn r1_batch_superpose_rollback() {
    let mut m = seeded();
    let before = snapshot(&m);
    let pairs = [(-1., -4.), (-1., -13.)].map(|(k, v)| (Fault::scalar(k), Fault::scalar(v)));
    let err = m.store_batch(&pairs).unwrap_err();
    assert_cause(err, "7 + 13");
    assert_eq!(snapshot(&m), before);
    assert_eq!(m.retrieve(&Fault::identity()).unwrap().raw_value.x(), 3.);
}
#[test]
fn r1_merge_superpose_rollback() {
    let mut m = seeded();
    let mut src = HolographicMemory::with_key_tracking(Default::default());
    src.store(&Fault::scalar(-1.), &Fault::scalar(-13.))
        .unwrap();
    let before = snapshot(&m);
    assert_cause(m.merge(&src).unwrap_err(), "3 + 13");
    assert_eq!(snapshot(&m), before);
    assert_eq!(m.retrieve(&Fault::identity()).unwrap().raw_value.x(), 3.);
}

#[test]
fn u2_transactional_bad_prefix_and_mixed_tracking() {
    let one = Cl3::one();
    let key = Cl3::scalar(2.) + Cl3::unit_vector(0);
    let v = Cl3::unit_vector(1);
    let mut m = HolographicMemory::with_key_tracking(Default::default());
    m.store(&key, &v).unwrap();
    let before = *m.trace();
    assert!(
        m.store_batch(&[(one, v), (one + Cl3::unit_vector(0), v)])
            .is_err(),
        "batch accepted a zero divisor"
    );
    assert_eq!(*m.trace(), before);
    assert_eq!(m.item_count(), 1);
    assert!(m.verify_consistency());
    let mut untracked = HolographicMemory::new(Default::default());
    m.merge(&untracked).unwrap();
    untracked.store(&one, &Cl3::zero()).unwrap();
    assert!(m.merge(&untracked).is_err());
    assert!(untracked.merge(&m).is_err());
    assert_eq!(*m.trace(), before);
    assert_eq!(m.item_count(), 1);
}
#[test]
fn u2_invalid_key_classes_and_imaginary_values() {
    type M = MAPAlgebra<2>;
    type F = FHRRAlgebra<2>;
    let mut m = HolographicMemory::new(Default::default());
    assert!(
        m.store(&M::new([2., 1.]), &M::identity()).is_err(),
        "accepted non-bipolar key"
    );
    let mut f = HolographicMemory::new(Default::default());
    assert!(f
        .store(&F::new([1., 0.], [0., 0.]), &F::identity())
        .is_err());
    assert!(f
        .store(&F::identity(), &F::new([0.; 2], [0., f64::NAN]))
        .is_err());
    assert_eq!(f.item_count(), 0);
}
