use anyhow::{Context, Result, bail};
use alpm::{Alpm, SigLevel, Usage, DownloadEvent, Progress};
use colored::Colorize;
use std::collections::HashMap;
use std::io::{self, Write};

use crate::config::{self, PacmanConfig};
use crate::cli::GlobalFlags;
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
    
    let arch_for_url = if !config.architectures.is_empty() {
        let first = config.architectures[0].as_str();
        if first == "auto" { utils::get_arch() } else { first.to_string() }
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
    
    if let Some(sig) = parse_siglevel(config.sig_level.as_ref()) {
        handle.set_default_siglevel(sig)?;
    }
    if let Some(sig) = parse_siglevel(config.local_file_sig_level.as_ref()) {
        handle.set_local_file_siglevel(sig)?;
    }
    if let Some(sig) = parse_siglevel(config.remote_file_sig_level.as_ref()) {
        handle.set_remote_file_siglevel(sig)?;
    }
    
    if !config.hook_dirs.is_empty() {
        handle.set_hookdirs(config.hook_dirs.iter().map(|s| s.as_str()))?;
    } else {
        handle.set_hookdirs(["/etc/pacman.d/hooks", "/usr/share/libalpm/hooks"].iter())?;
    }
    
    for repo in &config.repositories {
        let repo_sig = parse_siglevel(Some(&repo.sig_level)).unwrap_or(SigLevel::USE_DEFAULT);
        let db = handle.register_syncdb_mut(repo.name.as_str(), repo_sig)?;
        db.set_usage(Usage::ALL)?;
        for server in &repo.servers {
            let url = config::expand_server_url(server, &repo.name, &arch_for_url, &arch_v3, &arch_v4);
            db.add_server(url)?;
        }
    }
    
    for pattern in &global.overwrite {
        handle.add_overwrite_file(pattern.as_str())?;
    }

    // Progress callbacks
    handle.set_dl_cb(DownloadState::default(), |filename, event, state| {
        match event.event() {
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
            DownloadEvent::Completed(_) => {
                if state.note_complete(filename) {
                    println!("\r:: {} {}", "Downloaded".green().bold(), filename);
                }
            }
            _ => {}
        }
    });

    handle.set_progress_cb(TransState::default(), |progress, pkgname, percent, howmany, current, state| {
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
    });
    
    Ok(())
}

#[derive(Default)]
struct DownloadState {
    last_percent: HashMap<String, i32>,
    completed: HashMap<String, bool>,
}

impl DownloadState {
    fn note_start(&mut self, filename: &str) {
        self.last_percent.remove(filename);
        self.completed.remove(filename);
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
    let mut handle = Alpm::new(config.root_dir.as_str(), config.db_path.as_str())
        .context("Failed to initialize libalpm handle")?;
    configure_handle(&mut handle, &config, global)?;
    Ok(handle)
}

pub fn get_cache_dir(global: &GlobalFlags) -> Result<String> {
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
    Ok(config.cache_dir)
}

pub fn local_file_siglevel(global: &GlobalFlags) -> Result<SigLevel> {
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
    Ok(parse_siglevel(config.local_file_sig_level.as_ref()).unwrap_or(SigLevel::USE_DEFAULT))
}

pub fn find_sync_pkg<'a>(handle: &'a Alpm, name: &str) -> Result<&'a alpm::Package> {
    for db in handle.syncdbs().iter() {
        if let Ok(pkg) = db.pkg(name) {
            return Ok(pkg);
        }
    }
    bail!("error: target not found: {}", name)
}

pub fn find_local_pkg<'a>(handle: &'a Alpm, name: &str) -> Result<&'a alpm::Package> {
    let db = handle.localdb();
    let pkg = db.pkg(name)?;
    Ok(pkg)
}
