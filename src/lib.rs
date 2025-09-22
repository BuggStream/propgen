pub fn double(x: i64) -> i64 {
    x * 2
}

#[cfg(test)]
pub mod test {
    use crate::double;

    #[test]
    fn double_one() {
        let y = double(1);
        assert_eq!(y, 2);
    }
}