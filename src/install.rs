use anyhow::Result;
use alpm::TransFlag;
use std::fs;
use std::path::Path;
use colored::Colorize;

use crate::alpm_ops;
use crate::cli::{GlobalFlags, RemoveFlags};
use crate::utils;

fn trans_prepare_or_release(handle: &mut alpm::Alpm) -> Result<()> {
    let err_msg = match handle.trans_prepare() {
        Ok(()) => None,
        Err(err) => Some(err.to_string()),
    };
    if let Some(msg) = err_msg {
        let _ = handle.trans_release();
        if msg.to_lowercase().contains("architecture") {
            let allowed: Vec<String> = handle
                .architectures()
                .iter()
                .map(|a| a.to_string())
                .collect();
            let mut offenders = Vec::new();
            for pkg in handle.trans_add().iter() {
                let arch = pkg.arch().unwrap_or("unknown");
                if arch != "any" && !allowed.iter().any(|a| a == arch) {
                    offenders.push(format!("{} ({})", pkg.name(), arch));
                }
            }
            if !offenders.is_empty() {
                let details = format!(
                    "{}\nAllowed architectures: {}\nInvalid package architectures: {}",
                    msg,
                    allowed.join(", "),
                    offenders.join(", ")
                );
                return Err(anyhow::anyhow!(details));
            }
        }
        return Err(anyhow::anyhow!(msg));
    }
    Ok(())
}

pub fn install_packages(packages: &[String], global: &GlobalFlags) -> Result<()> {
    let mut handle = alpm_ops::init_handle(global)?;
    
    let mut flags = TransFlag::NONE;
    if global.needed {
        flags |= TransFlag::NEEDED;
    }
    if global.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if global.nodeps > 1 {
        flags |= TransFlag::NO_DEP_VERSION;
    }
    if global.noscriptlet {
        flags |= TransFlag::NO_SCRIPTLET;
    }
    handle.trans_init(flags)?;
    for name in packages {
        let pkg = alpm_ops::find_sync_pkg(&handle, name)?;
        handle
            .trans_add_pkg(pkg)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    
    println!("{}", "resolving dependencies...".cyan());
    println!("{}", "looking for conflicting packages...".cyan());
    trans_prepare_or_release(&mut handle)?;
    
    let to_install = handle.trans_add();
    if to_install.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        return Ok(());
    }
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with installation? [Y/n] ") {
        let _ = handle.trans_release();
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        apply_install_reasons(&handle, packages, global)?;
    }
    commit.map_err(|e| e.into())
}

pub fn install_local(global: &GlobalFlags, pkg_files: &[String]) -> Result<()> {
    let mut handle = alpm_ops::init_handle(global)?;
    let siglevel = alpm_ops::local_file_siglevel(global)?;
    
    let mut flags = TransFlag::NONE;
    if global.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if global.nodeps > 1 {
        flags |= TransFlag::NO_DEP_VERSION;
    }
    if global.noscriptlet {
        flags |= TransFlag::NO_SCRIPTLET;
    }
    
    handle.trans_init(flags)?;
    let mut names: Vec<String> = Vec::new();
    for file in pkg_files {
        let pkg = handle.pkg_load(file.as_str(), true, siglevel)?;
        names.push(pkg.name().to_string());
        handle
            .trans_add_pkg(pkg)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    
    println!("{}", "resolving dependencies...".cyan());
    println!("{}", "looking for conflicting packages...".cyan());
    trans_prepare_or_release(&mut handle)?;
    
    let to_install = handle.trans_add();
    if to_install.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        return Ok(());
    }
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with installation? [Y/n] ") {
        let _ = handle.trans_release();
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        apply_install_reasons(&handle, &names, global)?;
    }
    commit.map_err(|e| e.into())
}

pub fn remove_packages(packages: &[String], remove: &RemoveFlags, global: &GlobalFlags) -> Result<()> {
    let mut handle = alpm_ops::init_handle(global)?;
    
    let mut flags = TransFlag::NONE;
    if remove.recursive {
        flags |= TransFlag::RECURSE | TransFlag::UNNEEDED;
    }
    if remove.nosave {
        flags |= TransFlag::NO_SAVE;
    }
    if global.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if global.nodeps > 1 {
        flags |= TransFlag::NO_DEP_VERSION;
    }
    
    handle.trans_init(flags)?;
    for name in packages {
        let pkg = alpm_ops::find_local_pkg(&handle, name)?;
        handle.trans_remove_pkg(pkg)?;
    }
    
    println!("{}", "checking dependencies...".cyan());
    println!("{}", "looking for conflicting packages...".cyan());
    trans_prepare_or_release(&mut handle)?;
    
    let to_remove = handle.trans_remove();
    if to_remove.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        return Ok(());
    }
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with removal? [Y/n] ") {
        let _ = handle.trans_release();
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    commit.map_err(|e| e.into())
}

pub fn sync_install(
    global: &GlobalFlags,
    refresh: bool,
    upgrade: bool,
    targets: &[String],
) -> Result<()> {
    let mut handle = alpm_ops::init_handle(global)?;
    
    if refresh {
        println!(":: {}", "Synchronizing package databases...".cyan().bold());
        if global.test {
            println!(":: {}", "--test: skipping database update".yellow());
        } else {
            handle.syncdbs_mut().update(false)?;
        }
    }
    
    if !upgrade && targets.is_empty() {
        return Ok(());
    }
    
    let mut flags = TransFlag::NONE;
    if global.needed {
        flags |= TransFlag::NEEDED;
    }
    if global.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if global.nodeps > 1 {
        flags |= TransFlag::NO_DEP_VERSION;
    }
    if global.noscriptlet {
        flags |= TransFlag::NO_SCRIPTLET;
    }
    handle.trans_init(flags)?;
    if upgrade {
        println!(":: {}", "Starting full system upgrade...".cyan().bold());
        handle.sync_sysupgrade(false)?;
    }
    for name in targets {
        let pkg = alpm_ops::find_sync_pkg(&handle, name)?;
        handle
            .trans_add_pkg(pkg)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    
    println!("{}", "resolving dependencies...".cyan());
    println!("{}", "looking for conflicting packages...".cyan());
    trans_prepare_or_release(&mut handle)?;
    
    let to_add = handle.trans_add();
    if to_add.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        return Ok(());
    }
    
    println!("\n{}", "Packages to upgrade/install:".bold());
    let localdb = handle.localdb();
    for pkg in to_add.iter() {
        let old_ver = localdb
            .pkg(pkg.name())
            .map(|p| p.version().to_string())
            .unwrap_or_else(|_| "none".to_string());
        println!("  {} {} -> {}", pkg.name(), old_ver, pkg.version());
    }
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with installation? [Y/n] ") {
        let _ = handle.trans_release();
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        apply_install_reasons(&handle, targets, global)?;
    }
    commit.map_err(|e| e.into())
}

pub fn clean_cache(global: &GlobalFlags, level: u8) -> Result<()> {
    let cache_dir = alpm_ops::get_cache_dir(global)?;
    let cache_path = Path::new(&cache_dir);
    if !cache_path.exists() {
        return Ok(());
    }
    
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    
    let mut removed = 0usize;
    for entry in fs::read_dir(cache_path)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = match path.file_name().and_then(|s| s.to_str()) {
            Some(v) => v,
            None => continue,
        };
        if !file_name.contains(".pkg.tar") {
            continue;
        }
        
        let remove = if level >= 2 {
            true
        } else {
            match parse_pkg_filename(file_name) {
                Some((name, version)) => {
                    match localdb.pkg(name.as_str()) {
                        Ok(pkg) => pkg.version().to_string() != version,
                        Err(_) => true,
                    }
                }
                None => false,
            }
        };
        
        if remove {
            let _ = fs::remove_file(&path);
            removed += 1;
        }
    }
    
    if removed > 0 {
        println!(":: {} {}", "Cache cleaned:".green().bold(), format!("{} files removed", removed));
    } else {
        println!(":: {}", "Cache is clean".green().bold());
    }
    
    Ok(())
}

fn parse_pkg_filename(file_name: &str) -> Option<(String, String)> {
    let base = file_name.split(".pkg.tar").next()?;
    let mut parts = base.rsplitn(4, '-');
    let arch = parts.next()?;
    let rel = parts.next()?;
    let ver = parts.next()?;
    let name = parts.next()?;
    if arch.is_empty() || rel.is_empty() || ver.is_empty() || name.is_empty() {
        return None;
    }
    let version = format!("{}-{}", ver, rel);
    Some((name.to_string(), version))
}

fn apply_install_reasons(handle: &alpm::Alpm, targets: &[String], global: &GlobalFlags) -> Result<()> {
    if !global.asdeps && !global.asexplicit {
        return Ok(());
    }
    let reason = if global.asdeps {
        alpm::PackageReason::Depend
    } else {
        alpm::PackageReason::Explicit
    };
    let localdb = handle.localdb();
    for name in targets {
        if let Ok(pkg) = localdb.pkg(name.as_str()) {
            let _ = pkg.set_reason(reason);
        }
    }
    Ok(())
}
