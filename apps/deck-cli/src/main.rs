//! Deck command assembly. Card behavior remains owned by the pinned Shape services.
mod capabilities;
mod files;
mod material_return;
mod pdf;
mod project;
mod transfer;

use serde_json::{Value, json};
use std::{env, error::Error, fmt, path::PathBuf, process::ExitCode};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct Failure {
    code: &'static str,
    message: String,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for Failure {}
fn fail(code: &'static str, message: impl Into<String>) -> Box<dyn Error> {
    Box::new(Failure {
        code,
        message: message.into(),
    })
}

fn main() -> ExitCode {
    let (report, exit) = match run() {
        Ok(result) => (
            json!({"schema":"deck.cli.result/1","status":"ok","result":result}),
            ExitCode::SUCCESS,
        ),
        Err(error) => {
            let code = error
                .downcast_ref::<Failure>()
                .map_or("operation_failed", |e| e.code);
            (
                json!({"schema":"deck.cli.result/1","status":"error","code":code,"message":error.to_string()}),
                ExitCode::FAILURE,
            )
        }
    };
    println!("{report}");
    exit
}

fn run() -> Result<Value> {
    let mut args = env::args_os().skip(1);
    let command = args.next().ok_or_else(usage)?;
    if command == "capabilities" {
        if args.next().is_some() {
            return Err(usage());
        }
        return Ok(capabilities::report());
    }
    let project = PathBuf::from(args.next().ok_or_else(usage)?);
    if command == "status" || command == "hash-file" {
        if args.next().is_some() {
            return Err(usage());
        }
        return if command == "status" {
            project::status(&project)
        } else {
            files::hash_report(&project)
        };
    }
    let request = PathBuf::from(args.next().ok_or_else(usage)?);
    let extra: Vec<_> = args.collect();
    match command.to_str() {
        Some("create") if extra.is_empty() => project::create(&project, &request),
        Some("card-kit") if extra.is_empty() => project::card_action(&project, &request),
        Some("import-raster") if extra.is_empty() => project::import_raster(&project, &request),
        Some("import-material-return") if extra.is_empty() => {
            material_return::import(&project, &request)
        }
        Some("import-transfer") => {
            let expected = match extra.as_slice() {
                [] => None,
                [flag, digest] if flag == "--expected-blake3" => {
                    Some(digest.to_str().ok_or_else(usage)?)
                }
                _ => return Err(usage()),
            };
            transfer::import(&project, &request, expected)
        }
        Some("print-pdf") => {
            let desktop = match extra.as_slice() {
                [] => None,
                [flag, path] if flag == "--desktop" => Some(PathBuf::from(path)),
                _ => return Err(usage()),
            };
            pdf::export(&project, &request, desktop.as_deref())
        }
        _ => Err(usage()),
    }
}
fn usage() -> Box<dyn Error> {
    fail(
        "usage",
        "deck-cli capabilities | status <project> | hash-file <file> | create|card-kit|import-raster|import-material-return <project> <request.json> | import-transfer <new-project> <manifest.json> [--expected-blake3 HEX] | print-pdf <project> <request.json> [--desktop <deck-desktop>]",
    )
}
