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
    Evolve {
        target: String,
        #[arg(long)]
        approve: String,
        #[arg(long)]
        provider_fixture: PathBuf,
        #[arg(long, default_value_t = 3)]
        candidates: usize,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Test {
        #[arg(required_unless_present = "replay")]
        target: Option<String>,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long, conflicts_with = "replay")]
        candidate: Option<PathBuf>,
        #[arg(long, conflicts_with_all = ["target", "candidate"])]
        replay: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Explain {
        run_id: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    History {
        target: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Diff {
        revision_a: String,
        revision_b: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Apply {
        run_id: String,
        #[arg(long, required = true)]
        approve: bool,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Undo {
        #[arg(required_unless_present = "revision", conflicts_with = "revision")]
        run_id: Option<String>,
        #[arg(long, requires = "target", conflicts_with = "run_id")]
        revision: Option<String>,
        #[arg(long, requires = "revision")]
        target: Option<String>,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long)]
        json: bool,
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
        Some(Command::Evolve {
            target,
            approve,
            provider_fixture,
            candidates,
            workspace,
            json,
        }) => run_evolve(
            target,
            approve,
            provider_fixture,
            candidates,
            workspace,
            json,
        ),
        Some(Command::Test {
            target,
            workspace,
            candidate,
            replay,
            json,
        }) => run_test(target, workspace, candidate, replay, json),
        Some(Command::Explain {
            run_id,
            workspace,
            json,
        }) => run_explain(run_id, workspace, json),
        Some(Command::History {
            target,
            workspace,
            json,
        }) => run_history(target, workspace, json),
        Some(Command::Diff {
            revision_a,
            revision_b,
            workspace,
            json,
        }) => run_diff(revision_a, revision_b, workspace, json),
        Some(Command::Apply {
            run_id,
            approve,
            workspace,
            json,
        }) => run_apply(run_id, approve, workspace, json),
        Some(Command::Undo {
            run_id,
            revision,
            target,
            workspace,
            json,
        }) => run_undo(run_id, revision, target, workspace, json),
        None => ExitCode::SUCCESS,
    }
}

fn run_apply(run_id: String, approve: bool, workspace: PathBuf, output_json: bool) -> ExitCode {
    match sge_app::apply::apply_run(
        &workspace,
        &run_id,
        sge_app::apply::ApplyOptions {
            approved: approve,
            fault: None,
        },
    ) {
        Ok(outcome) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "run_id": outcome.run_id,
                        "target": outcome.target,
                        "previous_revision": outcome.previous_revision,
                        "applied_revision": outcome.applied_revision,
                        "record_path": outcome.record_path,
                    })
                );
            } else {
                println!(
                    "applied {} from {} as {}",
                    outcome.target, outcome.previous_revision, outcome.applied_revision
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn run_undo(
    run_id: Option<String>,
    revision: Option<String>,
    target: Option<String>,
    workspace: PathBuf,
    output_json: bool,
) -> ExitCode {
    let result = if let Some(run_id) = run_id {
        sge_app::undo::undo_run(&workspace, &run_id)
    } else {
        sge_app::undo::undo_revision(
            &workspace,
            target
                .as_deref()
                .expect("clap requires target with revision"),
            revision
                .as_deref()
                .expect("clap requires revision without run id"),
        )
    };
    match result {
        Ok(outcome) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "source": outcome.source,
                        "target": outcome.target,
                        "restored_revision": outcome.restored_revision,
                        "restoration_revision": outcome.restoration_revision,
                        "record_path": outcome.record_path,
                    })
                );
            } else {
                println!(
                    "restored {} from {} as {}",
                    outcome.target, outcome.restored_revision, outcome.restoration_revision
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn run_evolve(
    target: String,
    approve: String,
    provider_fixture: PathBuf,
    candidates: usize,
    workspace: PathBuf,
    output_json: bool,
) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("SGE-EVOLVE-001: failed to initialize runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    let options = sge_app::evolve::EvolveOptions {
        approve,
        provider_fixture,
        candidate_count: candidates,
    };
    match runtime.block_on(sge_app::evolve::evolve_workspace(
        &workspace, &target, options,
    )) {
        Ok(outcome) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "run_id": outcome.run_id,
                        "target": outcome.target,
                        "candidates": outcome.candidates,
                        "selected_candidate": outcome.selected_candidate,
                        "contract_path": outcome.contract_path,
                        "baseline_path": outcome.baseline_path,
                    })
                );
            } else {
                println!(
                    "evolution {} reached review ({})",
                    outcome.target, outcome.run_id
                );
                println!(
                    "selected candidate: {}",
                    outcome.selected_candidate.as_deref().unwrap_or("none")
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn run_test(
    target: Option<String>,
    workspace: PathBuf,
    candidate: Option<PathBuf>,
    replay: Option<String>,
    output_json: bool,
) -> ExitCode {
    if let Some(run_id) = replay {
        return match sge_app::replay::replay_run(&workspace, &run_id) {
            Ok(outcome) => {
                if output_json {
                    println!(
                        "{}",
                        json!({
                            "ok": outcome.matches,
                            "code": if outcome.matches { "OK" } else { "SGE-REPLAY-MISMATCH" },
                            "run_id": outcome.run_id,
                            "checked_evidence": outcome.checked_evidence,
                            "mismatches": outcome.mismatches,
                        })
                    );
                } else {
                    println!(
                        "replayed {}: matches={}, checked={}",
                        outcome.run_id, outcome.matches, outcome.checked_evidence
                    );
                }
                if outcome.matches {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(error) => print_error(error, output_json),
        };
    }

    let target = target.expect("clap requires target unless replay is present");
    match sge_app::evolve::test_workspace(&workspace, &target, candidate.as_deref()) {
        Ok(outcome) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "target": outcome.target,
                        "source_path": outcome.source_path,
                        "evaluation": outcome.evaluation,
                    })
                );
            } else {
                println!(
                    "tested {} at {}: task_success={:.3}, safety={:.3}",
                    outcome.target,
                    outcome.source_path.display(),
                    outcome.evaluation.metrics.task_success,
                    outcome.evaluation.metrics.safety,
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn run_explain(run_id: String, workspace: PathBuf, output_json: bool) -> ExitCode {
    match sge_app::explain::explain_run(&workspace, &run_id) {
        Ok(explanation) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "run_id": explanation.run_id,
                        "markdown": explanation.markdown,
                    })
                );
            } else {
                print!("{}", explanation.markdown);
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn run_history(target: String, workspace: PathBuf, output_json: bool) -> ExitCode {
    match sge_app::history::history_target(&workspace, &target) {
        Ok(entries) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "target": target,
                        "runs": entries,
                    })
                );
            } else {
                for entry in entries {
                    println!(
                        "{} {} selected={}",
                        entry.run_id,
                        entry.target,
                        entry.selected_candidate.as_deref().unwrap_or("none")
                    );
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn run_diff(
    revision_a: String,
    revision_b: String,
    workspace: PathBuf,
    output_json: bool,
) -> ExitCode {
    match sge_app::history::diff_revisions(&workspace, &revision_a, &revision_b) {
        Ok(diff) => {
            if output_json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "code": "OK",
                        "revision_a": revision_a,
                        "revision_b": revision_b,
                        "diff": diff,
                    })
                );
            } else {
                print!("{diff}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => print_error(error, output_json),
    }
}

fn print_error(error: sge_app::AppError, output_json: bool) -> ExitCode {
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
