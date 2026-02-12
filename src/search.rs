use anyhow::Result;
use alpm::{Package, PackageReason};
use colored::Colorize;

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

fn print_section_header(global: &GlobalFlags, title: &str, detail: Option<&str>) {
    if global.compact {
        return;
    }
    match detail {
        Some(d) => println!("{} {}", title.cyan().bold(), d.white().bold()),
        None => println!("{}", title.cyan().bold()),
    }
}

fn print_match_count(global: &GlobalFlags, count: usize) {
    if global.compact {
        return;
    }
    println!("\n{} {}", "Matches:".cyan().bold(), count.to_string().white().bold());
}

fn print_no_results() {
    println!("{}", "No results found".yellow());
}

fn print_pkg_row(
    global: &GlobalFlags,
    repo: Option<&str>,
    name: &str,
    version: &str,
    desc: Option<&str>,
    arch: Option<&str>,
    size: Option<i64>,
) {
    let name_text = name.green().bold();
    let ver_text = version.yellow();
    if let Some(r) = repo {
        println!("{}/{} {}", r.blue().bold(), name_text, ver_text);
    } else {
        println!("{} {}", name_text, ver_text);
    }
    if !global.compact {
        if let Some(d) = desc {
            println!("    {}", d.dimmed());
        }
    }
    if global.verbose {
        let arch_text = arch.unwrap_or("unknown");
        if let Some(s) = size {
            println!(
                "    {} {}  {} {}",
                "arch:".dimmed(),
                arch_text,
                "size:".dimmed(),
                s
            );
        } else {
            println!("    {} {}", "arch:".dimmed(), arch_text);
        }
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
    let mut count = 0usize;
    print_section_header(global, "Searching repositories for:", Some(&queries.join(" ")));
    for db in handle.syncdbs().iter() {
        let results = db.search(query_refs.iter())?;
        for pkg in results.iter() {
            let repo = pkg.db().map(|d| d.name()).unwrap_or(db.name());
            count += 1;
            print_pkg_row(
                global,
                Some(repo),
                pkg.name(),
                &pkg.version().to_string(),
                pkg.desc(),
                pkg.arch(),
                Some(pkg.isize()),
            );
            found = true;
        }
    }
    
    if !found {
        print_no_results();
    } else {
        print_match_count(global, count);
    }
    
    Ok(())
}

pub fn list_installed(global: &GlobalFlags) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    let mut count = 0usize;
    print_section_header(global, "Installed packages", None);
    
    for pkg in db.pkgs().iter() {
        print_pkg_row(
            global,
            None,
            pkg.name(),
            &pkg.version().to_string(),
            if global.verbose { pkg.desc() } else { None },
            pkg.arch(),
            Some(pkg.isize()),
        );
        count += 1;
    }
    print_match_count(global, count);
    
    Ok(())
}

pub fn search_installed(global: &GlobalFlags, queries: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    let query_refs: Vec<&str> = queries.iter().map(|s| s.as_str()).collect();
    
    let results = db.search(query_refs.iter())?;
    if results.is_empty() {
        print_no_results();
        return Ok(());
    }
    print_section_header(global, "Searching installed packages for:", Some(&queries.join(" ")));
    
    for pkg in results.iter() {
        print_pkg_row(
            global,
            None,
            pkg.name(),
            &pkg.version().to_string(),
            pkg.desc(),
            pkg.arch(),
            Some(pkg.isize()),
        );
    }
    print_match_count(global, results.len());
    
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
    print_section_header(global, "Package query", Some(&packages.join(" ")));
    
    for pkg_name in packages {
        let pkg = db.pkg(pkg_name.as_str())?;
        print_pkg_row(
            global,
            None,
            pkg.name(),
            &pkg.version().to_string(),
            if global.verbose { pkg.desc() } else { None },
            pkg.arch(),
            Some(pkg.isize()),
        );
    }
    
    Ok(())
}

pub fn list_package_files(global: &GlobalFlags, packages: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let db = handle.localdb();
    
    for pkg_name in packages {
        let pkg = db.pkg(pkg_name.as_str())
            .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", pkg_name))?;
        if !global.compact {
            println!("\n{} {}", "Files for".cyan().bold(), pkg.name().green().bold());
        }
        let files = pkg.files();
        let mut count = 0usize;
        for file in files.files() {
            let name = String::from_utf8_lossy(file.name()).to_string();
            if global.compact {
                println!("{} {}", pkg.name().green().bold(), name);
            } else {
                println!("  {}", name.dimmed());
            }
            count += 1;
        }
        if !global.compact {
            println!("{} {}", "File count:".cyan().bold(), count);
        }
    }
    
    Ok(())
}

pub fn list_manual_packages(global: &GlobalFlags) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    let syncdbs = handle.syncdbs();
    
    print_section_header(global, "Foreign packages", None);
    let mut count = 0usize;
    for pkg in localdb.pkgs().iter() {
        let mut found = false;
        for db in syncdbs.iter() {
            if db.pkg(pkg.name()).is_ok() {
                found = true;
                break;
            }
        }
        if !found {
            print_pkg_row(
                global,
                None,
                pkg.name(),
                &pkg.version().to_string(),
                if global.verbose { pkg.desc() } else { None },
                pkg.arch(),
                Some(pkg.isize()),
            );
            count += 1;
        }
    }
    if count == 0 {
        print_no_results();
    } else {
        print_match_count(global, count);
    }
    
    Ok(())
}

pub fn list_explicit_packages(global: &GlobalFlags) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    
    print_section_header(global, "Explicitly installed packages", None);
    let mut count = 0usize;
    for pkg in localdb.pkgs().iter() {
        if pkg.reason() == PackageReason::Explicit {
            print_pkg_row(
                global,
                None,
                pkg.name(),
                &pkg.version().to_string(),
                if global.verbose { pkg.desc() } else { None },
                pkg.arch(),
                Some(pkg.isize()),
            );
            count += 1;
        }
    }
    if count == 0 {
        print_no_results();
    } else {
        print_match_count(global, count);
    }
    
    Ok(())
}

pub fn query_explicit_packages(global: &GlobalFlags, packages: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    
    print_section_header(global, "Explicit package query", Some(&packages.join(" ")));
    let mut count = 0usize;
    for pkg_name in packages {
        let pkg = localdb
            .pkg(pkg_name.as_str())
            .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", pkg_name))?;
        if pkg.reason() == PackageReason::Explicit {
            print_pkg_row(
                global,
                None,
                pkg.name(),
                &pkg.version().to_string(),
                if global.verbose { pkg.desc() } else { None },
                pkg.arch(),
                Some(pkg.isize()),
            );
            count += 1;
        }
    }
    if count == 0 {
        print_no_results();
    } else {
        print_match_count(global, count);
    }
    
    Ok(())
}

pub fn query_reverse_dependencies(global: &GlobalFlags, packages: &[String]) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    
    for pkg_name in packages {
        let pkg = localdb
            .pkg(pkg_name.as_str())
            .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", pkg_name))?;
        let revdeps: Vec<String> = pkg.required_by().iter().map(|name| name.to_string()).collect();
        if revdeps.is_empty() {
            println!(
                "{} {}",
                pkg.name().green().bold(),
                "has no reverse dependencies".yellow()
            );
        } else {
            println!("{} {}", pkg.name().green().bold(), "is required by:".cyan().bold());
            for dep in revdeps {
                println!("  {}", dep.white().bold());
            }
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
                println!(
                    "{} {} {}",
                    input.white().bold(),
                    "is owned by".cyan().bold(),
                    pkg.name().green().bold()
                );
                found = true;
                break;
            }
        }
        
        if !found {
            eprintln!("error: {}", format!("No package owns {}", input).red());
        }
    }
    
    Ok(())
}
