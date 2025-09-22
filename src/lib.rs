use std::error::Error;
use std::path::PathBuf;
use crate::parser::parse_source_file;

pub mod parser;

pub fn parse_tests(files: &[PathBuf]) -> Result<(), Box<dyn Error>> {
    for file in files {
        let test_names = parse_source_file(file)?;
        for test_name in test_names {
            println!("{}", test_name);
        }
    }

    Ok(())
}

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
