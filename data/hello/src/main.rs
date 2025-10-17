use hello::{double, random::thrice_it};

fn main() {
    let doubled = double(5);
    let double_thriced = thrice_it(doubled);
    println!("{double_thriced}");
}
