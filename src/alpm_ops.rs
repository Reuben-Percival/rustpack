use alpm::{Alpm, DownloadEvent, Progress, SigLevel, Usage};
use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::cli::GlobalFlags;
use crate::config::{self, PacmanConfig};
use crate::utils;

pub(crate) fn parse_siglevel(input: Option<&String>) -> Option<SigLevel> {
    let s = input?;
    if s.is_empty() {
        return None;
    }
    if s.contains("UseDefault") {
        return Some(SigLevel::USE_DEFAULT);
    }
    if s.contains("Never") {
        return Some(SigLevel::NONE);
    }

    let mut level = SigLevel::NONE;
    let tokens = s.split_whitespace().collect::<Vec<_>>();
    for token in tokens {
        match token {
            "Required" => level |= SigLevel::PACKAGE,
            "Optional" => level |= SigLevel::PACKAGE_OPTIONAL,
            "DatabaseRequired" => level |= SigLevel::DATABASE,
            "DatabaseOptional" => level |= SigLevel::DATABASE_OPTIONAL,
            "DatabaseNever" => {}
            _ => {}
        }
    }
    if level == SigLevel::NONE {
        None
    } else {
        Some(level)
    }
}

fn configure_handle(handle: &mut Alpm, config: &PacmanConfig, global: &GlobalFlags) -> Result<()> {
    if let Some(ref cache_dir) = global.cache_dir {
        handle.add_cachedir(cache_dir.as_str())?;
    } else {
        handle.add_cachedir(config.cache_dir.as_str())?;
    }
    handle.set_check_space(config.check_space);

    if let Some(ref log_file) = config.log_file {
        handle.set_logfile(log_file.as_str())?;
    } else {
        handle.set_logfile("/var/log/pacman.log")?;
    }
    handle.set_use_syslog(config.use_syslog);

    if let Some(ref gpg_dir) = config.gpg_dir {
        handle.set_gpgdir(gpg_dir.as_str())?;
    } else {
        handle.set_gpgdir("/etc/pacman.d/gnupg")?;
    }
    if let Some(n) = global.parallel_downloads.or(config.parallel_downloads) {
        if n > 0 {
            handle.set_parallel_downloads(n);
        }
    }
    if global.disable_download_timeout || config.disable_download_timeout {
        handle.set_disable_dl_timeout(true);
    }

    let arch_for_url = if !config.architectures.is_empty() {
        let first = config.architectures[0].as_str();
        if first == "auto" {
            utils::get_arch()
        } else {
            first.to_string()
        }
    } else {
        utils::get_arch()
    };

    let (arch_base, arch_v3, arch_v4) = utils::arch_variants(arch_for_url.as_str());

    let mut added = std::collections::HashSet::new();
    let mut add_arch = |value: String| -> Result<()> {
        if added.insert(value.clone()) {
            handle.add_architecture(value.as_str())?;
        }
        Ok(())
    };
    if !config.architectures.is_empty() {
        for arch in &config.architectures {
            let value = if arch == "auto" {
                arch_for_url.clone()
            } else {
                arch.to_string()
            };
            add_arch(value)?;
        }
    } else {
        add_arch(arch_for_url.clone())?;
    }
    add_arch(arch_base.clone())?;
    add_arch(arch_v3.clone())?;
    add_arch(arch_v4.clone())?;

    if global.insecure_skip_signatures {
        handle.set_default_siglevel(SigLevel::NONE)?;
        handle.set_local_file_siglevel(SigLevel::NONE)?;
        handle.set_remote_file_siglevel(SigLevel::NONE)?;
    } else {
        if let Some(sig) = parse_siglevel(config.sig_level.as_ref()) {
            handle.set_default_siglevel(sig)?;
        }
        if let Some(sig) = parse_siglevel(config.local_file_sig_level.as_ref()) {
            handle.set_local_file_siglevel(sig)?;
        }
        if let Some(sig) = parse_siglevel(config.remote_file_sig_level.as_ref()) {
            handle.set_remote_file_siglevel(sig)?;
        }
    }

    if !config.hook_dirs.is_empty() {
        handle.set_hookdirs(config.hook_dirs.iter().map(|s| s.as_str()))?;
    } else {
        handle.set_hookdirs(["/etc/pacman.d/hooks", "/usr/share/libalpm/hooks"].iter())?;
    }

    for repo in &config.repositories {
        let repo_sig = if global.insecure_skip_signatures {
            SigLevel::NONE
        } else {
            parse_siglevel(Some(&repo.sig_level)).unwrap_or(SigLevel::USE_DEFAULT)
        };
        let db = handle.register_syncdb_mut(repo.name.as_str(), repo_sig)?;
        db.set_usage(Usage::ALL)?;
        for server in &repo.servers {
            let url =
                config::expand_server_url(server, &repo.name, &arch_for_url, &arch_v3, &arch_v4);
            db.add_server(url)?;
        }
    }

    for pattern in &global.overwrite {
        handle.add_overwrite_file(pattern.as_str())?;
    }

    // Progress callbacks
    handle.set_dl_cb(
        DownloadState::default(),
        |filename, event, state| match event.event() {
            DownloadEvent::Init(_) => {
                state.note_start(filename);
            }
            DownloadEvent::Progress(p) => {
                if p.total > 0 {
                    let percent = ((p.downloaded * 100) / p.total) as i32;
                    if state.should_print(filename, percent) {
                        let bar = progress_bar(percent, 28);
                        let line = format!(
                            ":: {} {} {} {}% ({}/{})",
                            "Downloading".cyan().bold(),
                            filename,
                            bar,
                            percent,
                            format_bytes(p.downloaded),
                            format_bytes(p.total)
                        );
                        print!("\r{}", line);
                        let _ = io::stdout().flush();
                    }
                }
            }
            DownloadEvent::Retry(r) => {
                if state.note_retry(filename) {
                    let mode = if r.resume { "resume" } else { "restart" };
                    println!(
                        "\r:: {} {} ({})",
                        "Retrying".yellow().bold(),
                        filename,
                        mode
                    );
                }
            }
            DownloadEvent::Completed(_) => {
                if state.note_complete(filename) {
                    println!("\r:: {} {}", "Downloaded".green().bold(), filename);
                }
            }
        },
    );

    handle.set_progress_cb(
        TransState::default(),
        |progress, pkgname, percent, howmany, current, state| {
            if state.should_print(progress, pkgname, percent, current, howmany) {
                let label = progress_label(progress);
                let bar = progress_bar(percent, 28);
                print!(
                    "\r:: {} {} {} {}% ({}/{})",
                    label.cyan().bold(),
                    pkgname,
                    bar,
                    percent,
                    current,
                    howmany
                );
                let _ = io::stdout().flush();
                if percent >= 100 {
                    println!();
                }
            }
        },
    );

    Ok(())
}

fn siglevel_is_weak(raw: &str) -> bool {
    let normalized = raw.to_ascii_lowercase();
    normalized.contains("never") || !normalized.contains("required")
}

fn enforce_strict_config(config: &PacmanConfig, global: &GlobalFlags) -> Result<()> {
    if !global.strict {
        return Ok(());
    }
    if let Some(sig) = config.sig_level.as_ref() {
        if siglevel_is_weak(sig) {
            bail!("error: --strict requires strong SigLevel; found '{}'", sig);
        }
    }
    if let Some(sig) = config.local_file_sig_level.as_ref() {
        if siglevel_is_weak(sig) {
            bail!(
                "error: --strict requires strong LocalFileSigLevel; found '{}'",
                sig
            );
        }
    }
    if let Some(sig) = config.remote_file_sig_level.as_ref() {
        if siglevel_is_weak(sig) {
            bail!(
                "error: --strict requires strong RemoteFileSigLevel; found '{}'",
                sig
            );
        }
    }
    for repo in &config.repositories {
        if siglevel_is_weak(repo.sig_level.as_str()) {
            bail!(
                "error: --strict requires strong repository SigLevel; repo '{}' has '{}'",
                repo.name,
                repo.sig_level
            );
        }
    }
    Ok(())
}

#[derive(Default)]
struct DownloadState {
    last_percent: HashMap<String, i32>,
    completed: HashMap<String, bool>,
    retries: HashMap<String, usize>,
}

impl DownloadState {
    fn note_start(&mut self, filename: &str) {
        self.last_percent.remove(filename);
        self.completed.remove(filename);
        self.retries.remove(filename);
    }

    fn should_print(&mut self, filename: &str, percent: i32) -> bool {
        let entry = self.last_percent.entry(filename.to_string()).or_insert(-1);
        if *entry == percent {
            false
        } else {
            *entry = percent;
            true
        }
    }

    fn note_complete(&mut self, filename: &str) -> bool {
        let entry = self.completed.entry(filename.to_string()).or_insert(false);
        if *entry {
            false
        } else {
            *entry = true;
            true
        }
    }

    fn note_retry(&mut self, filename: &str) -> bool {
        let entry = self.retries.entry(filename.to_string()).or_insert(0);
        *entry += 1;
        *entry <= 3
    }
}

#[derive(Default)]
struct TransState {
    last_key: Option<(Progress, String, i32, usize, usize)>,
}

impl TransState {
    fn should_print(
        &mut self,
        progress: Progress,
        pkgname: &str,
        percent: i32,
        current: usize,
        howmany: usize,
    ) -> bool {
        let key = (progress, pkgname.to_string(), percent, current, howmany);
        if self.last_key.as_ref() == Some(&key) {
            false
        } else {
            self.last_key = Some(key);
            true
        }
    }
}

fn progress_label(progress: Progress) -> &'static str {
    match progress {
        Progress::AddStart => "Installing",
        Progress::UpgradeStart => "Upgrading",
        Progress::DowngradeStart => "Downgrading",
        Progress::ReinstallStart => "Reinstalling",
        Progress::RemoveStart => "Removing",
        Progress::ConflictsStart => "Checking conflicts",
        Progress::DiskspaceStart => "Checking disk space",
        Progress::IntegrityStart => "Checking integrity",
        Progress::LoadStart => "Loading packages",
        Progress::KeyringStart => "Checking keys",
    }
}

fn progress_bar(percent: i32, width: usize) -> String {
    let pct = percent.clamp(0, 100) as usize;
    let filled = (pct * width) / 100;
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for _ in 0..filled {
        s.push('#');
    }
    for _ in filled..width {
        s.push('.');
    }
    s.push(']');
    s
}

fn format_bytes(bytes: i64) -> String {
    let mut value = bytes as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut idx = 0usize;
    while value >= 1024.0 && idx + 1 < units.len() {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{:.0} {}", value, units[idx])
    } else {
        format!("{:.1} {}", value, units[idx])
    }
}

pub fn init_handle(global: &GlobalFlags) -> Result<Alpm> {
    let config = effective_config(global)?;
    enforce_strict_config(&config, global)?;
    let mut handle = Alpm::new(config.root_dir.as_str(), config.db_path.as_str())
        .context("Failed to initialize libalpm handle")?;
    configure_handle(&mut handle, &config, global)?;
    Ok(handle)
}

pub fn get_cache_dir(global: &GlobalFlags) -> Result<String> {
    Ok(effective_config(global)?.cache_dir)
}

pub fn ensure_db_unlocked(global: &GlobalFlags) -> Result<()> {
    let config = effective_config(global)?;
    let lock_path = Path::new(&config.db_path).join("db.lck");
    if lock_path.exists() {
        bail!("database is locked (found {})", lock_path.to_string_lossy());
    }
    Ok(())
}

fn root_join(root: &str, rel: &str) -> String {
    let rel_trimmed = rel.trim_start_matches('/');
    if root == "/" {
        format!("/{}", rel_trimmed)
    } else {
        format!("{}/{}", root.trim_end_matches('/'), rel_trimmed)
    }
}

fn detect_distro(root: &str) -> String {
    let os_release = root_join(root, "/etc/os-release");
    let content = match fs::read_to_string(os_release) {
        Ok(v) => v.to_ascii_lowercase(),
        Err(_) => return "other".to_string(),
    };
    if content.contains("id=cachyos") {
        return "cachyos".to_string();
    }
    if content.contains("id=arch") || content.contains("id_like=arch") {
        return "arch".to_string();
    }
    "other".to_string()
}

pub fn preflight_transaction(global: &GlobalFlags) -> Result<()> {
    ensure_db_unlocked(global)?;
    let config = effective_config(global)?;
    let root = config.root_dir.as_str();
    let gpg_dir = config.gpg_dir.as_deref().unwrap_or("/etc/pacman.d/gnupg");
    let gpg_path = root_join(root, gpg_dir);
    let pubring_kbx = Path::new(&gpg_path).join("pubring.kbx");
    let pubring_gpg = Path::new(&gpg_path).join("pubring.gpg");
    let trustdb = Path::new(&gpg_path).join("trustdb.gpg");

    if !Path::new(&gpg_path).exists() {
        bail!(
            "keyring directory missing at {} (run pacman-key --init and repopulate keyrings)",
            gpg_path
        );
    }
    if !pubring_kbx.exists() && !pubring_gpg.exists() {
        bail!(
            "no keyring public keyring file in {} (expected pubring.kbx or pubring.gpg)",
            gpg_path
        );
    }
    if !trustdb.exists() {
        bail!("keyring trustdb missing at {}", trustdb.to_string_lossy());
    }

    let distro = detect_distro(root);
    let handle = Alpm::new(config.root_dir.as_str(), config.db_path.as_str())
        .context("Failed to initialize libalpm handle for preflight package checks")?;
    let localdb = handle.localdb();
    if localdb.pkg("archlinux-keyring").is_err() {
        bail!("archlinux-keyring is not installed in the local package database");
    }
    if distro == "cachyos" && localdb.pkg("cachyos-keyring").is_err() {
        bail!("cachyos-keyring is not installed in the local package database");
    }
    Ok(())
}

pub fn effective_config(global: &GlobalFlags) -> Result<PacmanConfig> {
    let mut config = config::parse_pacman_config("/etc/pacman.conf")?;
    if let Some(ref root_dir) = global.root_dir {
        config.root_dir = root_dir.clone();
    }
    if let Some(ref db_path) = global.db_path {
        config.db_path = db_path.clone();
    }
    if let Some(ref cache_dir) = global.cache_dir {
        config.cache_dir = cache_dir.clone();
    }
    Ok(config)
}

pub fn local_file_siglevel(global: &GlobalFlags) -> Result<SigLevel> {
    let config = effective_config(global)?;
    Ok(parse_siglevel(config.local_file_sig_level.as_ref()).unwrap_or(SigLevel::USE_DEFAULT))
}

pub fn find_sync_pkg<'a>(handle: &'a Alpm, name: &str) -> Result<&'a alpm::Package> {
    for db in handle.syncdbs().iter() {
        if let Ok(pkg) = db.pkg(name) {
            return Ok(pkg);
        }
    }
    let mut exact_providers: Vec<String> = Vec::new();
    let mut fuzzy_names: Vec<String> = Vec::new();
    let mut seen = HashSet::new();

    for db in handle.syncdbs().iter() {
        if let Ok(results) = db.search([name].iter()) {
            for pkg in results.iter() {
                let candidate = format!("{}/{}", db.name(), pkg.name());
                if seen.insert(candidate.clone()) {
                    fuzzy_names.push(candidate);
                }
                if fuzzy_names.len() >= 8 {
                    break;
                }
            }
        }
        for pkg in db.pkgs().iter() {
            for provide in pkg.provides().iter() {
                let provided_name = provide
                    .to_string()
                    .split(|c| c == '=' || c == '<' || c == '>')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                if provided_name == name {
                    let provider = format!("{}/{} (provides {})", db.name(), pkg.name(), name);
                    if seen.insert(provider.clone()) {
                        exact_providers.push(provider);
                    }
                    if exact_providers.len() >= 8 {
                        break;
                    }
                }
            }
            if exact_providers.len() >= 8 {
                break;
            }
        }
    }

    let mut msg = format!("error: target not found: {}", name);
    if !exact_providers.is_empty() {
        msg.push_str("\nPossible provider packages:");
        for p in exact_providers {
            msg.push_str(format!("\n  {}", p).as_str());
        }
    }
    if !fuzzy_names.is_empty() {
        msg.push_str("\nClosest repository matches:");
        for m in fuzzy_names {
            msg.push_str(format!("\n  {}", m).as_str());
        }
    }
    bail!(msg)
}

pub fn find_local_pkg<'a>(handle: &'a Alpm, name: &str) -> Result<&'a alpm::Package> {
    let db = handle.localdb();
    let pkg = db.pkg(name)?;
    Ok(pkg)
}
