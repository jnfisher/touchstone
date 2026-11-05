use touchstone_vb6::*;

#[test]
fn rnd_is_deterministic_and_in_range() {
    let mut a = Rnd::new();
    let mut b = Rnd::new();
    for _ in 0..1000 {
        let x = a.next_value();
        assert_eq!(x, b.next_value());
        assert!((0.0..1.0).contains(&x));
    }
}

#[test]
fn int_and_fix_agree_for_non_negative() {
    for i in 0..1000 {
        let x = f64::from(i) * 0.37;
        assert_eq!(int(x), fix(x));
    }
}

#[test]
fn idiv_and_mod_reassemble() {
    for a in -50..50 {
        for b in (-7..8).filter(|b| *b != 0) {
            let q = idiv(f64::from(a), f64::from(b)).unwrap();
            let r = vmod(f64::from(a), f64::from(b)).unwrap();
            assert_eq!(q * b + r, a);
        }
    }
}
