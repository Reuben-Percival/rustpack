use anyhow::Result;
use alpm::Package;

use crate::alpm_ops;
use crate::cli::GlobalFlags;

fn format_list<T: std::fmt::Display>(items: Vec<T>) -> String {
    if items.is_empty() {
        "None".to_string()
    } else {
        items
            .into_iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("  ")
    }
}

fn print_pkg_info(pkg: &Package, is_local: bool) {
    let db_name = pkg.db().map(|db| db.name()).unwrap_or("unknown");
    println!("Name            : {}", pkg.name());
    println!("Version         : {}", pkg.version());
    println!("Description     : {}", pkg.desc().unwrap_or("None"));
    println!("Architecture    : {}", pkg.arch().unwrap_or("unknown"));
    println!("URL             : {}", pkg.url().unwrap_or("None"));
    println!("Licenses        : {}", format_list(pkg.licenses().iter().collect()));
    println!("Groups          : {}", format_list(pkg.groups().iter().collect()));
    println!("Depends On      : {}", format_list(pkg.depends().iter().collect()));
    println!("Optional Deps   : {}", format_list(pkg.optdepends().iter().collect()));
    if is_local {
        println!("Install Reason  : {:?}", pkg.reason());
        println!("Install Date    : {}", pkg.install_date().unwrap_or(0));
        println!("Installed Size  : {}", pkg.isize());
    } else {
        println!("Repository      : {}", db_name);
        println!("Download Size   : {}", pkg.download_size());
        println!("Installed Size  : {}", pkg.isize());
    }
}

pub fn search_repos(global: &GlobalFlags, queries: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let query_refs: Vec<&str> = queries.iter().map(|s| s.as_str()).collect();
    
    let mut found = false;
    for db in handle.syncdbs().iter() {
        let results = db.search(query_refs.iter())?;
        for pkg in results.iter() {
            let repo = pkg.db().map(|d| d.name()).unwrap_or(db.name());
            println!("{}/{} {}", repo, pkg.name(), pkg.version());
            if let Some(desc) = pkg.desc() {
                println!("    {}", desc);
            }
            found = true;
        }
    }
    
    if !found {
        println!("No results found");
    }
    
    Ok(())
}

pub fn list_installed(global: &GlobalFlags) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    
    for pkg in db.pkgs().iter() {
        println!("{} {}", pkg.name(), pkg.version());
    }
    
    Ok(())
}

pub fn search_installed(global: &GlobalFlags, queries: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    let query_refs: Vec<&str> = queries.iter().map(|s| s.as_str()).collect();
    
    let results = db.search(query_refs.iter())?;
    if results.is_empty() {
        println!("No results found");
        return Ok(());
    }
    
    for pkg in results.iter() {
        println!("{} {}", pkg.name(), pkg.version());
        if let Some(desc) = pkg.desc() {
            println!("    {}", desc);
        }
    }
    
    Ok(())
}

pub fn show_package_info(global: &GlobalFlags, package_name: &str) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let pkg = alpm_ops::find_local_pkg(&handle, package_name)
        .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", package_name))?;
    print_pkg_info(pkg, true);
    Ok(())
}

pub fn show_sync_package_info(global: &GlobalFlags, package_name: &str) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let pkg = alpm_ops::find_sync_pkg(&handle, package_name)
        .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", package_name))?;
    print_pkg_info(pkg, false);
    Ok(())
}

pub fn query_packages(global: &GlobalFlags, packages: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    
    for pkg_name in packages {
        let pkg = db.pkg(pkg_name.as_str())?;
        println!("{} {}", pkg.name(), pkg.version());
    }
    
    Ok(())
}

pub fn list_package_files(global: &GlobalFlags, packages: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    
    for pkg_name in packages {
        let pkg = db.pkg(pkg_name.as_str())
            .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", pkg_name))?;
        let files = pkg.files();
        for file in files.files() {
            let name = String::from_utf8_lossy(file.name()).to_string();
            println!("{} {}", pkg.name(), name);
        }
    }
    
    Ok(())
}

pub fn list_manual_packages(global: &GlobalFlags) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    let syncdbs = handle.syncdbs();
    
    for pkg in localdb.pkgs().iter() {
        let mut found = false;
        for db in syncdbs.iter() {
            if db.pkg(pkg.name()).is_ok() {
                found = true;
                break;
            }
        }
        if !found {
            println!("{} {}", pkg.name(), pkg.version());
        }
    }
    
    Ok(())
}

fn normalize_query_path(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

pub fn query_owns(global: &GlobalFlags, paths: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    
    for input in paths {
        let query = normalize_query_path(input);
        let mut found = false;
        
        for pkg in db.pkgs().iter() {
            let files = pkg.files();
            if files.contains(query).is_some() {
                println!("{} {}", pkg.name(), input);
                found = true;
                break;
            }
        }
        
        if !found {
            eprintln!("error: No package owns {}", input);
        }
    }
    
    Ok(())
}
