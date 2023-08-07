use crate::settings::{
    param::{ParamsParser, SimpleParamHandle},
    ParsePayload,
};
use anyhow::{Context, Result};

pub struct RateOption {
    threads_info: ThreadsInfo,
}

#[derive(PartialEq, Debug)]
pub enum ThreadsInfo {
    Fixed {
        threads: u64,
        throttle: Option<u64>,
        fixed_rate: Option<u64>,
    },
    Auto {
        min_threads: u64,
        max_threads: u64,
        auto: bool,
    },
}

impl ThreadsInfo {
    fn print_settings(&self) {
        match &self {
            Self::Fixed {
                threads,
                throttle,
                fixed_rate,
            } => {
                println!("  Thread count: {}", threads);
                if let Some(throttle) = throttle {
                    println!("  OpsPer Sec: {}", throttle);
                }
                if let Some(fixed_rate) = fixed_rate {
                    println!("  Fixed: {}", fixed_rate);
                }
            }
            Self::Auto {
                min_threads,
                max_threads,
                auto,
            } => {
                println!("  Min threads: {}", min_threads);
                println!("  Max threads: {}", max_threads);
                println!("  auto: {}", auto);
            }
        }
    }
}

impl RateOption {
    pub const CLI_STRING: &str = "-rate";

    pub fn description() -> &'static str {
        "Thread count, rate limit or automatic mode (default is auto)"
    }

    pub fn parse(cl_args: &mut ParsePayload) -> Result<Self> {
        let params = cl_args.remove(Self::CLI_STRING).unwrap_or_default();
        let (parser, handles) = prepare_parser();
        parser.parse(params)?;
        Self::from_handles(handles)
    }

    pub fn print_help() {
        let (parser, _) = prepare_parser();
        parser.print_help();
    }

    pub fn print_settings(&self) {
        println!("Rate:");
        self.threads_info.print_settings();
    }

    fn from_handles(handles: RateParamHandles) -> Result<Self> {
        let threads = handles.threads.get_type::<u64>();
        let throttle = handles
            .throttle
            .get()
            .map(|th| parse_per_second(&th))
            .transpose()?;
        let fixed_rate = handles
            .fixed
            .get()
            .map(|fix| parse_per_second(&fix))
            .transpose()?;
        let min_threads = handles.threads_gte.get_type::<u64>();
        let max_threads = handles.threads_lte.get_type::<u64>();
        let auto = handles.auto.supplied_by_user();

        let threads_info = match (min_threads, max_threads) {
            (Some(min_threads), Some(max_threads)) => ThreadsInfo::Auto {
                min_threads,
                max_threads,
                auto,
            },
            _ => ThreadsInfo::Fixed {
                // SAFETY: The parameters are grouped in a way that this won't ever panic
                // when entering this branch.
                threads: threads.unwrap(),
                throttle,
                fixed_rate,
            },
        };

        Ok(Self { threads_info })
    }
}

struct RateParamHandles {
    pub threads: SimpleParamHandle,
    pub throttle: SimpleParamHandle,
    pub fixed: SimpleParamHandle,
    pub threads_gte: SimpleParamHandle,
    pub threads_lte: SimpleParamHandle,
    pub auto: SimpleParamHandle,
}

fn parse_per_second(arg: &str) -> Result<u64> {
    arg[..arg.len() - 2]
        .parse::<u64>()
        .with_context(|| format!("Value {} must end with '/s'", arg))
}

fn prepare_parser() -> (ParamsParser, RateParamHandles) {
    let mut parser = ParamsParser::new(RateOption::CLI_STRING);

    let threads = parser.simple_param(
        "threads=",
        r"^[0-9]+$",
        None,
        "run this many clients concurrently",
        true,
    );
    let throttle = parser.simple_param(
        "throttle=",
        r"^[0-9]+/s$",
        None,
        "throttle operations per second across all clients to a maximum rate (or less) with no implied schedule",
        false,
    );
    let fixed = parser.simple_param(
        "fixed=",
        r"^[0-9]+/s$",
        None,
        "expect fixed rate of operations per second across all clients with implied schedule",
        false,
    );
    let threads_gte = parser.simple_param(
        "threads>=",
        r"^[0-9]+$",
        Some("4"),
        "run at least this many clients concurrently",
        false,
    );
    let threads_lte = parser.simple_param(
        "threads<=",
        r"^[0-9]+$",
        Some("1000"),
        "run at most this many clients concurrently",
        false,
    );
    let auto = parser.simple_param(
        "auto",
        r"^$",
        None,
        "stop increasing threads once throughput saturates",
        false,
    );

    // $ ./cassandra-stress help -rate
    // Usage: -rate threads=? [throttle=?] [fixed=?]
    //  OR
    // Usage: -rate [threads>=?] [threads<=?] [auto]
    parser.group(&[&threads, &throttle, &fixed]);
    parser.group(&[&threads_gte, &threads_lte, &auto]);

    (
        parser,
        RateParamHandles {
            threads,
            throttle,
            fixed,
            threads_gte,
            threads_lte,
            auto,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::settings::option::{rate::ThreadsInfo, RateOption};

    use super::prepare_parser;

    #[test]
    fn rate_good_params_group_one_test() {
        let args = vec!["threads=100", "throttle=15/s"];
        let (parser, handles) = prepare_parser();

        assert!(parser.parse(args).is_ok());

        let params = RateOption::from_handles(handles).unwrap();
        assert_eq!(
            ThreadsInfo::Fixed {
                threads: 100,
                throttle: Some(15),
                fixed_rate: None
            },
            params.threads_info
        );
    }

    #[test]
    fn rate_good_params_group_two_test() {
        let args = vec!["threads<=200", "auto"];
        let (parser, handles) = prepare_parser();

        assert!(parser.parse(args).is_ok());

        let params = RateOption::from_handles(handles).unwrap();
        assert_eq!(
            ThreadsInfo::Auto {
                min_threads: 4,
                max_threads: 200,
                auto: true
            },
            params.threads_info
        )
    }

    #[test]
    fn rate_bad_params_test() {
        let args = vec!["threads<=200", "auto", "fixed=10/s"];
        let (parser, _) = prepare_parser();

        assert!(parser.parse(args).is_err());
    }
}
