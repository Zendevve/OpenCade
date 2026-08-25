use opencade_networking::{read_match_report, verify_match_reports, verify_playable_match_reports};
use opencade_protocol::MatchReport;
use serde::Serialize;
use std::path::Path;
use std::{env, process};

#[derive(Debug, Serialize)]
struct Failure {
    verified: bool,
    code: &'static str,
    message: String,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            verified: false,
            code,
            message: message.into(),
        }
    }
}

fn main() {
    match run() {
        Ok(output) => println!("{output}"),
        Err((failure, exit_code)) => {
            let output = serde_json::to_string(&failure).unwrap_or_else(|_| {
                "{\"verified\":false,\"code\":\"internal_error\",\"message\":\"failed to serialize error\"}".into()
            });
            eprintln!("{output}");
            process::exit(exit_code);
        }
    }
}

fn run() -> Result<String, (Failure, i32)> {
    let mut args = env::args().skip(1);
    let first_arg = args.next().ok_or_else(usage_failure)?;
    let (strict, first_path) = if first_arg == "--require-compatibility" {
        (true, args.next().ok_or_else(usage_failure)?)
    } else {
        (false, first_arg)
    };
    let second_path = args.next().ok_or_else(usage_failure)?;
    if args.next().is_some() {
        return Err(usage_failure());
    }

    let first = read_report(&first_path, "first")?;
    let second = read_report(&second_path, "second")?;
    let verification = if strict {
        verify_playable_match_reports(&first, &second)
    } else {
        verify_match_reports(&first, &second)
    }
    .map_err(|error| (Failure::new(error.code(), error.to_string()), 1))?;
    serde_json::to_string(&verification).map_err(|_| {
        (
            Failure::new("internal_error", "failed to serialize verification result"),
            2,
        )
    })
}

fn read_report(path: &str, label: &'static str) -> Result<MatchReport, (Failure, i32)> {
    read_match_report(Path::new(path))
        .map_err(|error| (Failure::new(error.code(), format!("{label} {error}")), 2))
}

fn usage_failure() -> (Failure, i32) {
    (
        Failure::new(
            "usage",
            "usage: opencade-match-verify [--require-compatibility] FIRST_REPORT.json SECOND_REPORT.json",
        ),
        2,
    )
}
