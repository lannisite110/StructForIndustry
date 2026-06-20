use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "sfi", about = "StructForIndustry platform CLI (v0.1)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Domain pack operations
    Domain {
        #[command(subcommand)]
        action: DomainAction,
    },
    /// Plugin scaffolding
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
enum DomainAction {
    /// List available domain packs
    List,
    /// Print active domain (from .sfi/active-domain)
    Use {
        /// Domain id to activate (writes .sfi/active-domain)
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// Create a new plugin scaffold under plugins/
    New {
        /// Plugin id, e.g. vision-2d
        name: String,
        /// Language: rust | julia
        #[arg(long, default_value = "julia")]
        lang: String,
    },
}

#[derive(Debug, Deserialize)]
struct DomainManifest {
    name: String,
    #[serde(default)]
    description: String,
}

fn repo_root() -> PathBuf {
    std::env::var("SFI_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let mut dir = std::env::current_dir().expect("cwd");
            loop {
                if dir.join("domains").is_dir() && dir.join("core").is_dir() {
                    return dir;
                }
                if !dir.pop() {
                    break;
                }
            }
            std::env::current_dir().expect("cwd")
        })
}

fn domains_dir(root: &Path) -> PathBuf {
    root.join("domains")
}

fn active_domain_path(root: &Path) -> PathBuf {
    root.join(".sfi").join("active-domain")
}

fn load_manifest(path: &Path) -> Result<DomainManifest, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_yaml::from_str(&text).map_err(|e| e.to_string())
}

fn cmd_domain_list(root: &Path) -> Result<(), String> {
    let domains = domains_dir(root);
    if !domains.is_dir() {
        return Err(format!("domains dir not found: {}", domains.display()));
    }

    let active = fs::read_to_string(active_domain_path(root))
        .ok()
        .map(|s| s.trim().to_string());

    for entry in fs::read_dir(&domains).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.yaml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = load_manifest(&manifest_path)?;
        let marker = if active.as_deref() == Some(manifest.name.as_str()) {
            " *"
        } else {
            ""
        };
        println!("{}{}\t{}", manifest.name, marker, manifest.description);
    }
    Ok(())
}

fn cmd_domain_use(root: &Path, name: Option<String>) -> Result<(), String> {
    match name {
        Some(id) => {
            let manifest_path = domains_dir(root).join(&id).join("manifest.yaml");
            if !manifest_path.is_file() {
                return Err(format!("unknown domain: {id}"));
            }
            let manifest = load_manifest(&manifest_path)?;
            let sfi_dir = root.join(".sfi");
            fs::create_dir_all(&sfi_dir).map_err(|e| e.to_string())?;
            fs::write(active_domain_path(root), &manifest.name).map_err(|e| e.to_string())?;
            println!("active domain: {}", manifest.name);
        }
        None => {
            let active = fs::read_to_string(active_domain_path(root))
                .map_err(|_| "no active domain (.sfi/active-domain missing)".to_string())?;
            println!("{}", active.trim());
        }
    }
    Ok(())
}

fn cmd_plugin_new(root: &Path, name: &str, lang: &str) -> Result<(), String> {
    let plugin_dir = root.join("plugins").join(name);
    if plugin_dir.exists() {
        return Err(format!("plugin already exists: {}", plugin_dir.display()));
    }
    fs::create_dir_all(&plugin_dir).map_err(|e| e.to_string())?;

    let readme = format!("# {name}\n\nScaffold created by `sfi plugin new`.\n\nLanguage: {lang}\n");
    fs::write(plugin_dir.join("README.md"), readme).map_err(|e| e.to_string())?;

    match lang {
        "julia" => {
            fs::write(
                plugin_dir.join("Project.toml"),
                format!(
                    r#"[name]
{name}

[deps]
JSON3 = "1"
SFIMathKernel = {{ path = "../../core/math-kernel" }}
Sockets = "1"
"#
                ),
            )
            .map_err(|e| e.to_string())?;
            fs::write(
                plugin_dir.join("server.jl"),
                r#"#!/usr/bin/env julia
# Plugin sidecar — plugin wire v1 over Unix socket.
include("server_impl.jl")
"#,
            )
            .map_err(|e| e.to_string())?;
        }
        "rust" => {
            fs::write(
                plugin_dir.join("Cargo.toml"),
                format!(
                    r#"[package]
name = "{name}"
version = "0.0.1"
edition = "2021"

[dependencies]
sfi-plugin-host = {{ path = "../../core/plugin-host" }}
tokio = {{ version = "1", features = ["io-util", "macros", "net", "rt-multi-thread"] }}
"#
                ),
            )
            .map_err(|e| e.to_string())?;
        }
        other => return Err(format!("unsupported lang: {other} (use rust or julia)")),
    }

    println!("created {}", plugin_dir.display());
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let root = repo_root();

    let result = match cli.command {
        Commands::Domain { action } => match action {
            DomainAction::List => cmd_domain_list(&root),
            DomainAction::Use { name } => cmd_domain_use(&root, name),
        },
        Commands::Plugin { action } => match action {
            PluginAction::New { name, lang } => cmd_plugin_new(&root, &name, &lang),
        },
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_root_finds_monorepo() {
        let root = repo_root();
        assert!(root.join("domains").is_dir());
    }
}
