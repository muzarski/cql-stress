use std::time::Duration;

use anyhow::Result;

pub trait Parsable: Sized {
    type Parsed;

    fn parse(s: &str) -> Result<Self::Parsed>;

    // Used only to print the same help message as cassandra-stress does for boolean flags.
    fn is_bool() -> bool {
        false
    }
}

/// Simple macro for checking if value `s` matches the regex `regex_str`.
/// Returns error if the value didn't match.
macro_rules! ensure_regex {
    ($s:ident, $regex_str:expr) => {
        let regex = regex::Regex::new($regex_str).unwrap();
        anyhow::ensure!(
            regex.is_match($s),
            "Invalid value {}; must match pattern {}",
            $s,
            $regex_str
        )
    };
}

// Implementation of Parsable for common types.

impl Parsable for u64 {
    type Parsed = u64;

    fn parse(s: &str) -> Result<Self::Parsed> {
        ensure_regex!(s, r"^[0-9]+$");
        Ok(s.parse::<u64>().unwrap())
    }
}

impl Parsable for f64 {
    type Parsed = f64;

    fn parse(s: &str) -> Result<Self::Parsed> {
        ensure_regex!(s, r"^0\.[0-9]+$");
        Ok(s.parse::<f64>().unwrap())
    }
}

impl Parsable for bool {
    type Parsed = bool;

    fn parse(s: &str) -> Result<Self::Parsed> {
        anyhow::ensure!(
            s.is_empty(),
            "Invalid value {}. Boolean flag cannot have any value.",
            s
        );

        Ok(true)
    }

    fn is_bool() -> bool {
        true
    }
}

impl Parsable for String {
    type Parsed = String;

    fn parse(s: &str) -> Result<Self::Parsed> {
        Ok(s.to_owned())
    }
}

impl Parsable for Duration {
    type Parsed = Duration;

    fn parse(s: &str) -> Result<Self::Parsed> {
        ensure_regex!(s, r"^[0-9]+[smh]$");

        let parse_duration_unit = |unit: char| -> u64 {
            match unit {
                's' => 1,
                'm' => 60,
                'h' => 60 * 60,
                _ => panic!("Invalid duration unit: {unit}"),
            }
        };

        let multiplier = parse_duration_unit(s.chars().last().unwrap());
        let value = Duration::from_secs(s[0..s.len() - 1].parse::<u64>().unwrap() * multiplier);
        Ok(value)
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Wrapper over the parameter's value matching pattern "[0-9]+[bmk]?".
/// [bmk] suffix denotes the multiplier. One of billion, million or thousand.
pub struct Count;

impl Parsable for Count {
    type Parsed = u64;

    fn parse(s: &str) -> Result<Self::Parsed> {
        ensure_regex!(s, r"^[0-9]+[bmk]?$");

        let parse_operation_count_unit = |unit: char| -> u64 {
            match unit {
                'k' => 1_000,
                'm' => 1_000_000,
                'b' => 1_000_000_000,
                _ => panic!("Invalid operation count unit: {unit}"),
            }
        };

        let last = s.chars().last().unwrap();
        let mut multiplier = 1;
        let mut number_slice = s;
        if last.is_alphabetic() {
            multiplier = parse_operation_count_unit(last);
            number_slice = &s[0..s.len() - 1];
        }
        let value = number_slice.parse::<u64>().unwrap() * multiplier;
        Ok(value)
    }
}

pub struct Rate;

impl Parsable for Rate {
    type Parsed = u64;

    fn parse(s: &str) -> Result<Self::Parsed> {
        ensure_regex!(s, r"^[0-9]+/s$");

        let value = s[..s.len() - 2].parse::<u64>().unwrap();
        Ok(value)
    }
}

pub struct CommaDelimitedList;

impl Parsable for CommaDelimitedList {
    type Parsed = Vec<String>;

    fn parse(s: &str) -> Result<Self::Parsed> {
        ensure_regex!(s, r"^[^=,]+(,[^=,]+)*$");
        Ok(s.split(',').map(|e| e.to_owned()).collect())
    }
}
