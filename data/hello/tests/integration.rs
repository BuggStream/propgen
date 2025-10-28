use proptest::proptest;
use hello::double;
use propgen_macro::propgen;

#[propgen]
#[test]
fn double_twice() {
    let doubled = double(double(1));
    assert_eq!(doubled, 4);
}

#[propgen]
fn other_stuff(x: String) -> String {
    format!("... {x}")
}

proptest! {
    #[test]
    fn double_proptest(x in (i64::MIN / 2)..(i64::MAX / 2)) {
        assert_eq!(double(x), x * 2);
    }
}

