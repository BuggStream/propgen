pub fn thrice_it(x: i64) -> i64 {
    x * 3
}


#[cfg(test)]
mod test {
    use crate::random::thrice_it;

    #[test]
    fn check_thrice() {
        let y = thrice_it(4);
        assert_eq!(y, 12);
    }
}
