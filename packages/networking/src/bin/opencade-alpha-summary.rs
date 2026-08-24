use opencade_networking::{read_match_report, summarize_match_reports};
use std::path::{Path, PathBuf};
use std::{env, fs, process};

const MAX_CAMPAIGN_REPORTS: usize = 100;

fn main() {
    match run() {
        Ok(gate_passed) if gate_passed => {}
        Ok(_) => process::exit(1),
        Err(message) => {
            eprintln!("{message}");
            process::exit(2);
        }
    }
}

fn run() -> Result<bool, String> {
    let mut args = env::args_os().skip(1);
    let directory = args.next().ok_or_else(usage)?;
    if args.next().is_some() {
        return Err(usage());
    }

    let mut paths = report_paths(Path::new(&directory))?;
    paths.sort();
    if paths.len() > MAX_CAMPAIGN_REPORTS {
        return Err(format!(
            "campaign contains {} JSON reports; maximum is {MAX_CAMPAIGN_REPORTS}",
            paths.len()
        ));
    }

    let mut reports = Vec::with_capacity(paths.len());
    for path in paths {
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("<invalid filename>");
        let report = read_match_report(&path)
            .map_err(|error| format!("{filename}: {} ({})", error, error.code()))?;
        reports.push(report);
    }
    let summary = summarize_match_reports(&reports);
    let output = serde_json::to_string_pretty(&summary)
        .map_err(|_| "failed to serialize campaign summary".to_string())?;
    println!("{output}");
    Ok(summary.gate_passed)
}

fn report_paths(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries =
        fs::read_dir(directory).map_err(|_| "campaign directory is unreadable".to_string())?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| "campaign directory is unreadable".to_string())?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "json")
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn usage() -> String {
    "usage: opencade-alpha-summary REPORT_DIRECTORY".into()
}
