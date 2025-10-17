use hello::double;
use propgen_macro::propgen;

#[propgen]
#[test]
fn double_twice() {
    let doubled = double(double(1));
    assert_eq!(doubled, 4);
}

