use anyhow::{Result, bail};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use crate::alpm_ops;
use crate::cli::GlobalFlags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Distro {
    Arch,
    CachyOS,
    Other,
}

struct Report {
    ok: usize,
    warn: usize,
    fail: usize,
    checks: Vec<(String, &'static str)>,
    json: bool,
}

impl Report {
    fn new(json: bool) -> Self {
        Self {
            ok: 0,
            warn: 0,
            fail: 0,
            checks: Vec::new(),
            json,
        }
    }

    fn ok(&mut self, label: &str) {
        self.ok += 1;
        self.checks.push((label.to_string(), "ok"));
        if !self.json {
            println!("{} {}", "[OK]".green().bold(), label);
        }
    }

    fn warn(&mut self, label: &str) {
        self.warn += 1;
        self.checks.push((label.to_string(), "warn"));
        if !self.json {
            println!("{} {}", "[WARN]".yellow().bold(), label);
        }
    }

    fn fail(&mut self, label: &str) {
        self.fail += 1;
        self.checks.push((label.to_string(), "fail"));
        if !self.json {
            println!("{} {}", "[FAIL]".red().bold(), label);
        }
    }
}

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn root_join(root: &str, rel: &str) -> PathBuf {
    let trimmed = rel.trim_start_matches('/');
    if root == "/" {
        Path::new("/").join(trimmed)
    } else {
        Path::new(root).join(trimmed)
    }
}

fn detect_distro(root: &str) -> Distro {
    let path = root_join(root, "/etc/os-release");
    let content = match fs::read_to_string(path) {
        Ok(v) => v.to_ascii_lowercase(),
        Err(_) => return Distro::Other,
    };
    if content.contains("id=cachyos") || content.contains("id_like=\"cachyos") {
        return Distro::CachyOS;
    }
    if content.contains("id=arch") || content.contains("id_like=arch") {
        return Distro::Arch;
    }
    Distro::Other
}

fn active_package_manager_processes() -> Vec<String> {
    let mut found = Vec::new();
    let self_pid = process::id().to_string();
    let entries = match fs::read_dir("/proc") {
        Ok(v) => v,
        Err(_) => return found,
    };
    let names = [
        "pacman", "rustpack", "paru", "yay", "pamac", "trizen", "pikaur",
    ];
    for entry in entries.flatten() {
        let pid = match entry.file_name().to_str() {
            Some(v) if v.chars().all(|c| c.is_ascii_digit()) => v.to_string(),
            _ => continue,
        };
        if pid == self_pid {
            continue;
        }
        let comm = fs::read_to_string(entry.path().join("comm"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let cmdline = fs::read(entry.path().join("cmdline"))
            .map(|v| {
                String::from_utf8_lossy(&v)
                    .replace('\0', " ")
                    .to_ascii_lowercase()
            })
            .unwrap_or_default();
        if names
            .iter()
            .any(|n| comm.contains(n) || cmdline.contains(n))
        {
            found.push(format!("pid {} ({})", pid, comm.trim()));
        }
    }
    found
}

fn try_fix_stale_lock(lock_path: &Path) -> Result<bool> {
    if !lock_path.exists() {
        return Ok(false);
    }
    if !active_package_manager_processes().is_empty() {
        return Ok(false);
    }
    fs::remove_file(lock_path)?;
    Ok(true)
}

pub fn run(global: &GlobalFlags) -> Result<()> {
    let config = alpm_ops::effective_config(global)?;
    let mut report = Report::new(global.json);
    let distro = detect_distro(config.root_dir.as_str());
    let distro_name = match distro {
        Distro::Arch => "Arch Linux",
        Distro::CachyOS => "CachyOS",
        Distro::Other => "Unknown/Other",
    };
    if !global.json {
        println!("{}", "rustpack doctor".bold());
        println!("Detected distro profile: {}", distro_name);
        println!("Root: {}", config.root_dir);
        println!("DBPath: {}", config.db_path);
        println!("CacheDir: {}", config.cache_dir);
        if global.doctor_fix {
            println!("{}", "Fix mode: enabled (safe fixes only)".yellow().bold());
        }
        println!();
    }

    if Path::new(config.root_dir.as_str()).exists() {
        report.ok("Root directory exists");
    } else {
        report.fail("Root directory does not exist");
    }

    if Path::new(config.db_path.as_str()).exists() {
        report.ok("Package database path exists");
    } else {
        report.fail("Package database path does not exist");
    }

    let local_db = Path::new(config.db_path.as_str()).join("local");
    if local_db.exists() {
        report.ok("Local package database exists");
    } else {
        report.fail("Local package database is missing");
    }

    let lock_path = Path::new(config.db_path.as_str()).join("db.lck");
    if lock_path.exists() {
        if global.doctor_fix {
            match try_fix_stale_lock(&lock_path) {
                Ok(true) => report.ok("No active database lock file (removed stale lock)"),
                Ok(false) => {
                    let active = active_package_manager_processes();
                    if active.is_empty() {
                        report.warn("Database lock file exists (could not auto-remove lock)");
                    } else {
                        report.warn(
                            format!(
                                "Database lock file exists and active package manager processes were found: {}",
                                active.join(", ")
                            )
                            .as_str(),
                        );
                    }
                }
                Err(_) => report.warn("Database lock file exists (failed to remove stale lock)"),
            }
        } else {
            report
                .warn("Database lock file exists (possible active package manager or stale lock)");
        }
    } else {
        report.ok("No active database lock file");
    }

    if Path::new(config.cache_dir.as_str()).exists() {
        report.ok("Package cache path exists");
    } else if global.doctor_fix {
        match fs::create_dir_all(config.cache_dir.as_str()) {
            Ok(_) => report.ok("Package cache path exists (created missing directory)"),
            Err(_) => report.warn("Package cache path is missing (failed to create directory)"),
        }
    } else {
        report.warn("Package cache path is missing");
    }

    let gpg_dir = config.gpg_dir.as_deref().unwrap_or("/etc/pacman.d/gnupg");
    let gpg_dir_path = root_join(config.root_dir.as_str(), gpg_dir);
    if gpg_dir_path.exists() {
        report.ok("GPG directory exists");
    } else {
        report.fail("GPG directory is missing");
    }

    let pubring_kbx = gpg_dir_path.join("pubring.kbx");
    let pubring_gpg = gpg_dir_path.join("pubring.gpg");
    if pubring_kbx.exists() || pubring_gpg.exists() {
        report.ok("Keyring public keyring file exists");
    } else {
        report.fail("No keyring public keyring file found (pubring.kbx/pubring.gpg)");
    }

    let trustdb = gpg_dir_path.join("trustdb.gpg");
    if trustdb.exists() {
        report.ok("Keyring trustdb exists");
    } else {
        report.warn("Keyring trustdb.gpg not found");
    }

    if config.repositories.is_empty() {
        report.fail("No repositories configured");
    } else {
        report.ok("Repositories configured");
    }

    let mut repo_names = Vec::new();
    let mut insecure_server_count = 0usize;
    for repo in &config.repositories {
        repo_names.push(repo.name.to_ascii_lowercase());
        if repo.servers.is_empty() {
            report.fail(format!("Repository '{}' has no servers", repo.name).as_str());
            continue;
        }
        let https_count = repo
            .servers
            .iter()
            .filter(|s| s.starts_with("https://"))
            .count();
        if https_count == 0 {
            insecure_server_count += 1;
        }
        if https_count < repo.servers.len() {
            report.warn(
                format!(
                    "Repository '{}' has non-HTTPS mirrors ({} of {})",
                    repo.name,
                    repo.servers.len() - https_count,
                    repo.servers.len()
                )
                .as_str(),
            );
        }
    }

    if insecure_server_count == 0 && !config.repositories.is_empty() {
        report.ok("All repositories include HTTPS mirrors");
    }

    match distro {
        Distro::Arch => {
            let has_core = repo_names.iter().any(|r| r == "core");
            let has_extra = repo_names.iter().any(|r| r == "extra");
            if has_core && has_extra {
                report.ok("Arch baseline repositories present (core, extra)");
            } else {
                report.warn("Arch baseline repositories missing one of: core, extra");
            }
        }
        Distro::CachyOS => {
            let has_cachy_repo = repo_names.iter().any(|r| r.contains("cachyos"));
            if has_cachy_repo {
                report.ok("CachyOS repositories detected");
            } else {
                report.warn(
                    "No CachyOS repositories detected (expected for optimized CachyOS setups)",
                );
            }
            let has_arch_opt = config
                .repositories
                .iter()
                .flat_map(|r| r.servers.iter())
                .any(|s| {
                    s.contains("$arch_v3")
                        || s.contains("$arch_v4")
                        || s.contains("x86_64_v3")
                        || s.contains("x86_64_v4")
                });
            if has_arch_opt {
                report.ok("Architecture-optimized mirror patterns detected (v3/v4)");
            } else {
                report.warn("No architecture-optimized mirror patterns detected (v3/v4)");
            }
        }
        Distro::Other => {
            report
                .warn("Distro is not recognized as Arch/CachyOS; only generic checks were applied");
        }
    }

    if global.json {
        let checks = report
            .checks
            .iter()
            .map(|(label, status)| {
                format!(
                    "{{\"status\":\"{}\",\"label\":\"{}\"}}",
                    status,
                    json_escape(label)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{{\"profile\":\"{}\",\"root\":\"{}\",\"dbpath\":\"{}\",\"cachedir\":\"{}\",\"summary\":{{\"ok\":{},\"warn\":{},\"fail\":{}}},\"checks\":[{}]}}",
            json_escape(distro_name),
            json_escape(config.root_dir.as_str()),
            json_escape(config.db_path.as_str()),
            json_escape(config.cache_dir.as_str()),
            report.ok,
            report.warn,
            report.fail,
            checks
        );
    } else {
        println!();
        println!(
            "{} ok={} warn={} fail={}",
            "Doctor summary:".bold(),
            report.ok,
            report.warn,
            report.fail
        );
    }

    if report.fail > 0 {
        if global.json {
            bail!("__RUSTPACK_JSON_DOCTOR_FAILED__");
        }
        bail!("doctor found failing checks");
    }
    Ok(())
}
