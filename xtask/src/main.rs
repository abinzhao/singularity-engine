use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use sge_protocol::schemas::v1_schema_files;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);

    match (args.next().as_deref(), args.next()) {
        (Some("architecture"), None) => check_architecture(),
        (Some("schema" | "schemas"), None) => write_schemas(),
        _ => {
            eprintln!("usage: cargo xtask <architecture|schema>");
            std::process::exit(2);
        }
    }
}

fn check_architecture() -> Result<(), Box<dyn Error>> {
    let packages = workspace_packages()?;
    let violations = architecture_violations(&packages);

    if violations.is_empty() {
        println!("architecture check passed: workspace dependency direction matches ADR-001 rules");
        return Ok(());
    }

    eprintln!("architecture check failed:");
    for violation in violations {
        eprintln!("- {violation}");
    }
    std::process::exit(1);
}

fn write_schemas() -> Result<(), Box<dyn Error>> {
    let schema_dir = workspace_root().join("schemas/v1");
    fs::create_dir_all(&schema_dir)?;

    for schema_file in v1_schema_files()? {
        fs::write(
            schema_dir.join(schema_file.file_name()),
            schema_file.contents(),
        )?;
    }

    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[derive(Debug)]
struct WorkspacePackage {
    name: String,
    manifest_dir: PathBuf,
    dependencies: Vec<String>,
}

fn workspace_packages() -> Result<Vec<WorkspacePackage>, Box<dyn Error>> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .current_dir(workspace_root())
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed with status {}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    parse_workspace_packages(&output.stdout)
}

fn parse_workspace_packages(metadata: &[u8]) -> Result<Vec<WorkspacePackage>, Box<dyn Error>> {
    let metadata: Value = serde_json::from_slice(metadata)?;
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or("cargo metadata missing workspace_members")?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata missing packages")?;

    let mut workspace_packages = Vec::new();

    for package in packages {
        let Some(id) = package.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !workspace_members
            .iter()
            .any(|member| member.as_str() == Some(id))
        {
            continue;
        }

        let name = required_string(package, "name")?.to_string();
        let manifest_path = required_string(package, "manifest_path")?;
        let manifest_dir = PathBuf::from(manifest_path)
            .parent()
            .ok_or("cargo metadata manifest_path has no parent")?
            .to_path_buf();
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or("cargo metadata package missing dependencies")?
            .iter()
            .filter_map(|dependency| dependency.get("name").and_then(Value::as_str))
            .map(str::to_string)
            .collect();

        workspace_packages.push(WorkspacePackage {
            name,
            manifest_dir,
            dependencies,
        });
    }

    Ok(workspace_packages)
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("cargo metadata missing string field {key}").into())
}

fn architecture_violations(packages: &[WorkspacePackage]) -> Vec<String> {
    let mut violations = Vec::new();

    for package in packages {
        for dependency in &package.dependencies {
            if package.name == "sge-domain" && dependency == "sge-store" {
                violations.push("sge-domain must not depend on sge-store".to_string());
            }
            if package.name == "sge-domain" && dependency == "sge-cli" {
                violations.push("sge-domain must not depend on sge-cli".to_string());
            }
            if package.name == "sge-protocol" && dependency == "sge-cli" {
                violations.push("sge-protocol must not depend on sge-cli".to_string());
            }
            if is_core_crate(package)
                && (is_concrete_adapter_name(dependency)
                    || packages
                        .iter()
                        .find(|candidate| candidate.name == *dependency)
                        .is_some_and(is_concrete_adapter))
            {
                violations.push(format!(
                    "{} must not depend on adapter crate {}",
                    package.name, dependency
                ));
            }
        }
    }

    violations
}

fn is_core_crate(package: &WorkspacePackage) -> bool {
    package.name.starts_with("sge-") && !is_concrete_adapter(package)
}

fn is_concrete_adapter(package: &WorkspacePackage) -> bool {
    is_concrete_adapter_name(&package.name)
        || package
            .manifest_dir
            .components()
            .any(|component| component.as_os_str() == "adapters")
}

fn is_concrete_adapter_name(name: &str) -> bool {
    name.starts_with("adapter-") || name.contains("adapter-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_allows_current_core_direction() {
        let packages = vec![
            package("sge-cli", "crates/sge-cli", &["sge-app"]),
            package("sge-app", "crates/sge-app", &["sge-domain", "sge-store"]),
            package("sge-domain", "crates/sge-domain", &[]),
            package("sge-protocol", "crates/sge-protocol", &["sge-domain"]),
            package("sge-store", "crates/sge-store", &["sge-domain"]),
        ];

        assert!(architecture_violations(&packages).is_empty());
    }

    #[test]
    fn architecture_rejects_domain_store_and_cli_dependencies() {
        let packages = vec![package(
            "sge-domain",
            "crates/sge-domain",
            &["sge-store", "sge-cli"],
        )];

        let violations = architecture_violations(&packages);

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("sge-domain must not depend on sge-store"))
        );
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("sge-domain must not depend on sge-cli"))
        );
    }

    #[test]
    fn architecture_rejects_core_crates_that_depend_on_adapters() {
        let packages = vec![
            package("sge-app", "crates/sge-app", &["sge-adapter-github"]),
            package("adapter-git", "adapters/git", &[]),
            package("sge-store", "crates/sge-store", &["adapter-git"]),
        ];

        let violations = architecture_violations(&packages);

        assert!(violations.iter().any(|violation| {
            violation.contains("sge-app must not depend on adapter crate sge-adapter-github")
        }));
        assert!(violations.iter().any(|violation| {
            violation.contains("sge-store must not depend on adapter crate adapter-git")
        }));
    }

    #[test]
    fn architecture_rejects_protocol_cli_dependency() {
        let packages = vec![package("sge-protocol", "crates/sge-protocol", &["sge-cli"])];

        let violations = architecture_violations(&packages);

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("sge-protocol must not depend on sge-cli"))
        );
    }

    fn package(name: &str, manifest_dir: &str, dependencies: &[&str]) -> WorkspacePackage {
        WorkspacePackage {
            name: name.to_string(),
            manifest_dir: PathBuf::from(manifest_dir),
            dependencies: dependencies
                .iter()
                .map(|dependency| dependency.to_string())
                .collect(),
        }
    }
}
