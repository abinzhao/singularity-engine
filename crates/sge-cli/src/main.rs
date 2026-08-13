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
    Scan {
        target: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, conflicts_with = "goal")]
        approve: Option<String>,
        #[arg(long, conflicts_with = "approve")]
        goal: Option<String>,
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
        Some(Command::Scan {
            target,
            workspace,
            json,
            approve,
            goal,
        }) => run_scan(target, workspace, approve, goal, json),
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

fn run_scan(
    target: String,
    workspace: PathBuf,
    approve: Option<String>,
    goal: Option<String>,
    output_json: bool,
) -> ExitCode {
    let options = sge_app::scan::ScanOptions { approve, goal };
    match sge_app::scan::scan_workspace(&workspace, &target, options) {
        Ok(outcome) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "run_id": outcome.run_id,
                        "target": outcome.target,
                        "proposals": outcome.proposals,
                        "proposals_path": outcome.proposals_path,
                        "contract_path": outcome.contract_path,
                    })
                );
            } else {
                println!("scan {} ({})", outcome.target, outcome.run_id);
                for proposal in outcome.proposals {
                    println!(
                        "{}: {} [risk={}, confidence={:.2}]",
                        proposal.id, proposal.title, proposal.risk, proposal.confidence
                    );
                }
                println!("proposals: {}", outcome.proposals_path.display());
                if let Some(contract_path) = outcome.contract_path {
                    println!("contract: {}", contract_path.display());
                }
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
