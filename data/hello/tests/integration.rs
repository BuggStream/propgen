use proptest::prelude::*;
use hello::double;

const INPUT: i64 = 1;

#[propgen_macro::propgen]
#[propgen_macro::propgen_input(INPUT)]
#[test]
fn double_twice() {
    let doubled = double(double(INPUT));
    assert_eq!(doubled, 4 * INPUT);
}

fn other_stuff(x: String) -> String {
    format!("... {x}")
}
