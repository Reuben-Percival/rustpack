use anyhow::{Context, Result, bail};
use alpm::{Alpm, SigLevel, Usage};

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
    
    if !config.architectures.is_empty() {
        for arch in &config.architectures {
            let value = if arch == "auto" {
                arch_for_url.clone()
            } else {
                arch.to_string()
            };
            handle.add_architecture(value.as_str())?;
        }
    } else {
        handle.add_architecture(arch_for_url.as_str())?;
    }
    
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
            let url = config::expand_server_url(server, &repo.name, &arch_for_url);
            db.add_server(url)?;
        }
    }
    
    for pattern in &global.overwrite {
        handle.add_overwrite_file(pattern.as_str())?;
    }
    
    Ok(())
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
