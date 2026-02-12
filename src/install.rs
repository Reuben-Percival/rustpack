use anyhow::Result;
use alpm::TransFlag;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use colored::Colorize;

use crate::alpm_ops;
use crate::cli::{GlobalFlags, RemoveFlags};
use crate::history;
use crate::utils;

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

fn format_net_bytes(bytes: i64) -> String {
    if bytes >= 0 {
        format!("+{}", format_bytes(bytes))
    } else {
        format!("-{}", format_bytes(-bytes))
    }
}

fn add_summary(handle: &alpm::Alpm) -> (usize, usize, i64, i64) {
    let to_add = handle.trans_add();
    let localdb = handle.localdb();
    
    let mut install_count = 0usize;
    let mut upgrade_count = 0usize;
    let mut total_download = 0i64;
    let mut net_change = 0i64;
    
    for pkg in to_add.iter() {
        total_download += pkg.download_size();
        let old_size = localdb.pkg(pkg.name()).map(|p| p.isize()).unwrap_or(0);
        if old_size > 0 {
            upgrade_count += 1;
        } else {
            install_count += 1;
        }
        net_change += pkg.isize() - old_size;
    }
    
    (install_count, upgrade_count, total_download.max(0), net_change)
}

fn print_add_summary(handle: &alpm::Alpm, global: &GlobalFlags) {
    let (install_count, upgrade_count, total_download, net_change) = add_summary(handle);
    if global.compact {
        println!(
            "summary: install={} upgrade={} download={} net={}",
            install_count,
            upgrade_count,
            format_bytes(total_download),
            format_net_bytes(net_change)
        );
        return;
    }
    println!("\n{}", "Transaction Summary".bold());
    println!("  Install: {}", install_count);
    println!("  Upgrade: {}", upgrade_count);
    println!("  Download Size: {}", format_bytes(total_download));
    println!("  Net Installed Size: {}", format_net_bytes(net_change));
}

fn remove_summary(handle: &alpm::Alpm) -> (usize, i64) {
    let to_remove = handle.trans_remove();
    let mut remove_count = 0usize;
    let mut reclaimed = 0i64;
    
    for pkg in to_remove.iter() {
        remove_count += 1;
        reclaimed += pkg.isize();
    }
    
    (remove_count, reclaimed.max(0))
}

fn print_remove_summary(handle: &alpm::Alpm, global: &GlobalFlags) {
    let (remove_count, reclaimed) = remove_summary(handle);
    if global.compact {
        println!(
            "summary: remove={} reclaimed={} net={}",
            remove_count,
            format_bytes(reclaimed),
            format_net_bytes(-reclaimed)
        );
        return;
    }
    println!("\n{}", "Transaction Summary".bold());
    println!("  Remove: {}", remove_count);
    println!("  Reclaimed Space: {}", format_bytes(reclaimed));
    println!("  Net Installed Size: {}", format_net_bytes(-reclaimed));
}

fn warn_remove_breakage(handle: &alpm::Alpm, packages: &[String], remove: &RemoveFlags) -> Result<()> {
    if remove.recursive {
        return Ok(());
    }
    let localdb = handle.localdb();
    let targets: HashSet<&str> = packages.iter().map(|s| s.as_str()).collect();
    let mut warned = false;
    
    for pkg_name in packages {
        let pkg = localdb
            .pkg(pkg_name.as_str())
            .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", pkg_name))?;
        let mut dependents = Vec::new();
        for name in pkg.required_by().iter() {
            if !targets.contains(name) {
                dependents.push(name.to_string());
            }
        }
        if !dependents.is_empty() {
            warned = true;
            eprintln!(
                "warning: removing '{}' may break dependent packages: {}",
                pkg_name,
                dependents.join(", ")
            );
        }
    }
    if warned {
        eprintln!("hint: use -Rs to remove packages with their unneeded dependencies.");
    }
    Ok(())
}

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
    if global.verbose {
        println!(":: verbose: operation=install targets={}", packages.join(" "));
    }
    handle.trans_init(flags)?;
    for name in packages {
        let pkg = alpm_ops::find_sync_pkg(&handle, name)?;
        handle
            .trans_add_pkg(pkg)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    
    if !global.compact {
        println!("{}", "resolving dependencies...".cyan());
        println!("{}", "looking for conflicting packages...".cyan());
    }
    trans_prepare_or_release(&mut handle)?;
    
    let to_install = handle.trans_add();
    if to_install.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        let _ = history::record(global, "install", "noop", packages, "no packages to install");
        return Ok(());
    }
    print_add_summary(&handle, global);
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with installation? [Y/n] ") {
        let _ = handle.trans_release();
        let _ = history::record(global, "install", "cancelled", packages, "user cancelled transaction");
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        let _ = history::record(global, "install", "dry-run", packages, "commit skipped by --test");
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        apply_install_reasons(&handle, packages, global)?;
        let _ = history::record(global, "install", "success", packages, "transaction committed");
    } else {
        let _ = history::record(global, "install", "failed", packages, "transaction commit failed");
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
    if global.verbose {
        println!(":: verbose: operation=install-local files={}", pkg_files.join(" "));
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
    
    if !global.compact {
        println!("{}", "resolving dependencies...".cyan());
        println!("{}", "looking for conflicting packages...".cyan());
    }
    trans_prepare_or_release(&mut handle)?;
    
    let to_install = handle.trans_add();
    if to_install.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        let _ = history::record(global, "install-local", "noop", &names, "no packages to install");
        return Ok(());
    }
    print_add_summary(&handle, global);
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with installation? [Y/n] ") {
        let _ = handle.trans_release();
        let _ = history::record(global, "install-local", "cancelled", &names, "user cancelled transaction");
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        let _ = history::record(global, "install-local", "dry-run", &names, "commit skipped by --test");
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        apply_install_reasons(&handle, &names, global)?;
        let _ = history::record(global, "install-local", "success", &names, "transaction committed");
    } else {
        let _ = history::record(global, "install-local", "failed", &names, "transaction commit failed");
    }
    commit.map_err(|e| e.into())
}

pub fn remove_packages(packages: &[String], remove: &RemoveFlags, global: &GlobalFlags) -> Result<()> {
    let mut handle = alpm_ops::init_handle(global)?;
    if global.verbose {
        println!(":: verbose: operation=remove targets={}", packages.join(" "));
    }
    warn_remove_breakage(&handle, packages, remove)?;
    
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
    
    if !global.compact {
        println!("{}", "checking dependencies...".cyan());
        println!("{}", "looking for conflicting packages...".cyan());
    }
    trans_prepare_or_release(&mut handle)?;
    
    let to_remove = handle.trans_remove();
    if to_remove.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        let _ = history::record(global, "remove", "noop", packages, "no packages to remove");
        return Ok(());
    }
    print_remove_summary(&handle, global);
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with removal? [Y/n] ") {
        let _ = handle.trans_release();
        let _ = history::record(global, "remove", "cancelled", packages, "user cancelled transaction");
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        let _ = history::record(global, "remove", "dry-run", packages, "commit skipped by --test");
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        let _ = history::record(global, "remove", "success", packages, "transaction committed");
    } else {
        let _ = history::record(global, "remove", "failed", packages, "transaction commit failed");
    }
    commit.map_err(|e| e.into())
}

pub fn sync_install(
    global: &GlobalFlags,
    refresh: bool,
    upgrade: bool,
    targets: &[String],
) -> Result<()> {
    let mut handle = alpm_ops::init_handle(global)?;
    if global.verbose {
        println!(":: verbose: operation=sync refresh={} upgrade={} targets={}", refresh, upgrade, targets.join(" "));
    }
    
    if refresh {
        if !global.compact {
            println!(":: {}", "Synchronizing package databases...".cyan().bold());
        }
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
        if !global.compact {
            println!(":: {}", "Starting full system upgrade...".cyan().bold());
        }
        handle.sync_sysupgrade(false)?;
    }
    for name in targets {
        let pkg = alpm_ops::find_sync_pkg(&handle, name)?;
        handle
            .trans_add_pkg(pkg)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    
    if !global.compact {
        println!("{}", "resolving dependencies...".cyan());
        println!("{}", "looking for conflicting packages...".cyan());
    }
    trans_prepare_or_release(&mut handle)?;
    
    let to_add = handle.trans_add();
    if to_add.is_empty() {
        let _ = handle.trans_release();
        println!(" {}", "there is nothing to do".yellow());
        let _ = history::record(global, "sync", "noop", targets, "no package changes");
        return Ok(());
    }
    print_add_summary(&handle, global);
    if !global.compact {
        println!("\n{}", "Packages to upgrade/install:".bold());
    }
    let localdb = handle.localdb();
    if !global.compact {
        for pkg in to_add.iter() {
            let old_ver = localdb
                .pkg(pkg.name())
                .map(|p| p.version().to_string())
                .unwrap_or_else(|_| "none".to_string());
            println!("  {} {} -> {}", pkg.name(), old_ver, pkg.version());
        }
    }
    
    if !global.test && !global.noconfirm && !utils::confirm_action("\n:: Proceed with installation? [Y/n] ") {
        let _ = handle.trans_release();
        let _ = history::record(global, "sync", "cancelled", targets, "user cancelled transaction");
        return Ok(());
    }
    
    if global.test {
        println!(":: {}", "--test: skipping commit".yellow());
        let _ = handle.trans_release();
        let _ = history::record(global, "sync", "dry-run", targets, "commit skipped by --test");
        return Ok(());
    }
    
    let commit = handle.trans_commit();
    let _ = handle.trans_release();
    if commit.is_ok() {
        apply_install_reasons(&handle, targets, global)?;
        let _ = history::record(global, "sync", "success", targets, "transaction committed");
    } else {
        let _ = history::record(global, "sync", "failed", targets, "transaction commit failed");
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
