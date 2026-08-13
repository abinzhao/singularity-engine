use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "sge", about = "SINGULARITY ENGINE")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Import {
        path: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init { path, json }) => run_init(path, json),
        Some(Command::Import {
            path,
            json,
            workspace,
        }) => run_import(path, workspace, json),
        None => ExitCode::SUCCESS,
    }
}

fn run_init(path: PathBuf, output_json: bool) -> ExitCode {
    match sge_app::init::initialize(&path) {
        Ok(workspace) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "root": workspace.root,
                        "manifest": workspace.manifest_path,
                        "store": workspace.store_path,
                    })
                );
            } else {
                println!("initialized workspace at {}", workspace.root.display());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if output_json {
                eprintln!(
                    "{}",
                    json!({
                        "ok": false,
                        "code": error.code(),
                        "message": error.to_string(),
                    })
                );
            } else {
                eprintln!("{}: {error}", error.code());
            }
            ExitCode::FAILURE
        }
    }
}

fn run_import(path: PathBuf, workspace: PathBuf, output_json: bool) -> ExitCode {
    match sge_app::import::import_artifact(&workspace, &path) {
        Ok(imported) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "target": imported.target,
                        "revision": imported.revision,
                        "warnings": imported.warnings,
                    })
                );
            } else {
                println!(
                    "imported {} (revision {})",
                    imported.target, imported.revision
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if output_json {
                eprintln!(
                    "{}",
                    json!({
                        "ok": false,
                        "code": error.code(),
                        "message": error.to_string(),
                    })
                );
            } else {
                eprintln!("{}: {error}", error.code());
            }
            ExitCode::FAILURE
        }
    }
}
