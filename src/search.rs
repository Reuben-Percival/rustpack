use anyhow::Result;
use alpm::{Package, PackageReason};
use colored::Colorize;
use std::collections::VecDeque;

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

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn json_array(items: Vec<String>) -> String {
    items
        .into_iter()
        .map(|v| format!("\"{}\"", json_escape(&v)))
        .collect::<Vec<_>>()
        .join(",")
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

fn pkg_info_json(pkg: &Package, is_local: bool) -> String {
    let db_name = pkg.db().map(|db| db.name()).unwrap_or("unknown");
    let mut out = String::new();
    out.push_str("{");
    out.push_str(format!("\"name\":\"{}\"", json_escape(pkg.name())).as_str());
    out.push_str(format!(",\"version\":\"{}\"", json_escape(pkg.version().as_ref())).as_str());
    out.push_str(format!(",\"description\":\"{}\"", json_escape(pkg.desc().unwrap_or("None"))).as_str());
    out.push_str(format!(",\"architecture\":\"{}\"", json_escape(pkg.arch().unwrap_or("unknown"))).as_str());
    out.push_str(format!(",\"url\":\"{}\"", json_escape(pkg.url().unwrap_or("None"))).as_str());
    let licenses = pkg.licenses().iter().map(|v| v.to_string()).collect::<Vec<_>>();
    let groups = pkg.groups().iter().map(|v| v.to_string()).collect::<Vec<_>>();
    let depends = pkg.depends().iter().map(|v| v.to_string()).collect::<Vec<_>>();
    let optdepends = pkg.optdepends().iter().map(|v| v.to_string()).collect::<Vec<_>>();
    out.push_str(format!(",\"licenses\":[{}]", json_array(licenses)).as_str());
    out.push_str(format!(",\"groups\":[{}]", json_array(groups)).as_str());
    out.push_str(format!(",\"depends\":[{}]", json_array(depends)).as_str());
    out.push_str(format!(",\"optdepends\":[{}]", json_array(optdepends)).as_str());
    if is_local {
        out.push_str(format!(",\"install_reason\":\"{:?}\"", pkg.reason()).as_str());
        out.push_str(format!(",\"install_date\":{}", pkg.install_date().unwrap_or(0)).as_str());
        out.push_str(format!(",\"installed_size\":{}", pkg.isize()).as_str());
    } else {
        out.push_str(format!(",\"repository\":\"{}\"", json_escape(db_name)).as_str());
        out.push_str(format!(",\"download_size\":{}", pkg.download_size()).as_str());
        out.push_str(format!(",\"installed_size\":{}", pkg.isize()).as_str());
    }
    out.push('}');
    out
}

fn print_pkg_info(pkg: &Package, is_local: bool, global: &GlobalFlags) {
    if global.json {
        println!("{}", pkg_info_json(pkg, is_local));
        return;
    }
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
    print_pkg_info(pkg, true, global);
    Ok(())
}

pub fn show_sync_package_info(global: &GlobalFlags, package_name: &str) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let pkg = alpm_ops::find_sync_pkg(&handle, package_name)
        .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", package_name))?;
    print_pkg_info(pkg, false, global);
    Ok(())
}

pub fn show_local_package_infos(global: &GlobalFlags, package_names: &[String]) -> Result<()> {
    if !global.json {
        for pkg in package_names {
            show_package_info(global, pkg)?;
        }
        return Ok(());
    }
    let handle = alpm_ops::init_handle(global)?;
    let mut items = Vec::new();
    for package_name in package_names {
        let pkg = alpm_ops::find_local_pkg(&handle, package_name)
            .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", package_name))?;
        items.push(pkg_info_json(pkg, true));
    }
    println!("[{}]", items.join(","));
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
    if global.json {
        let mut rows = Vec::new();
        for pkg in localdb.pkgs().iter() {
            if pkg.reason() == PackageReason::Explicit {
                rows.push(format!(
                    "{{\"name\":\"{}\",\"version\":\"{}\",\"description\":\"{}\",\"architecture\":\"{}\",\"installed_size\":{}}}",
                    json_escape(pkg.name()),
                    json_escape(pkg.version().as_ref()),
                    json_escape(pkg.desc().unwrap_or("")),
                    json_escape(pkg.arch().unwrap_or("unknown")),
                    pkg.isize()
                ));
            }
        }
        println!("[{}]", rows.join(","));
        return Ok(());
    }
    
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
    if global.json {
        let mut rows = Vec::new();
        for pkg_name in packages {
            let pkg = localdb
                .pkg(pkg_name.as_str())
                .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", pkg_name))?;
            if pkg.reason() == PackageReason::Explicit {
                rows.push(format!(
                    "{{\"name\":\"{}\",\"version\":\"{}\",\"description\":\"{}\",\"architecture\":\"{}\",\"installed_size\":{}}}",
                    json_escape(pkg.name()),
                    json_escape(pkg.version().as_ref()),
                    json_escape(pkg.desc().unwrap_or("")),
                    json_escape(pkg.arch().unwrap_or("unknown")),
                    pkg.isize()
                ));
            }
        }
        println!("[{}]", rows.join(","));
        return Ok(());
    }
    
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

pub fn explain_why(global: &GlobalFlags, package_name: &str) -> Result<()> {
    let handle = alpm_ops::init_handle(global)?;
    let localdb = handle.localdb();
    let target = localdb
        .pkg(package_name)
        .map_err(|_| anyhow::anyhow!("error: package '{}' was not found", package_name))?;

    println!(
        "{} {}",
        "Why is installed:".cyan().bold(),
        package_name.green().bold()
    );

    if target.reason() == PackageReason::Explicit {
        println!(
            "{} {}",
            package_name.green().bold(),
            "is explicitly installed".yellow()
        );
        return Ok(());
    }

    let mut queue: VecDeque<Vec<String>> = VecDeque::new();
    queue.push_back(vec![package_name.to_string()]);
    let mut chains: Vec<Vec<String>> = Vec::new();
    let max_depth = 10usize;
    let max_chains = 8usize;

    while let Some(path) = queue.pop_front() {
        if chains.len() >= max_chains {
            break;
        }
        if path.len() > max_depth {
            continue;
        }

        let current = path.last().map(|s| s.as_str()).unwrap_or(package_name);
        let pkg = match localdb.pkg(current) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let revdeps: Vec<String> = pkg.required_by().iter().map(|n| n.to_string()).collect();
        for parent in revdeps {
            if chains.len() >= max_chains {
                break;
            }
            if path.iter().any(|p| p == &parent) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(parent.clone());

            if let Ok(parent_pkg) = localdb.pkg(parent.as_str()) {
                if parent_pkg.reason() == PackageReason::Explicit {
                    chains.push(next_path);
                    continue;
                }
            }
            queue.push_back(next_path);
        }
    }

    if chains.is_empty() {
        let revdeps: Vec<String> = target.required_by().iter().map(|n| n.to_string()).collect();
        if revdeps.is_empty() {
            println!(
                "{}",
                "No reverse dependencies found; it may be an orphan dependency.".yellow()
            );
        } else {
            println!(
                "{}",
                "No explicit install chain found within search depth.".yellow()
            );
        }
        return Ok(());
    }

    for (idx, chain) in chains.iter().enumerate() {
        println!("\n{} {}", "Chain".cyan().bold(), (idx + 1).to_string().white().bold());
        for (i, node) in chain.iter().enumerate() {
            if i + 1 == chain.len() {
                println!("  {} {}", node.green().bold(), "(explicit)".yellow());
            } else {
                println!("  {}", node.white().bold());
            }
            if i + 1 != chain.len() {
                println!("    {}", "required by".dimmed());
            }
        }
    }
    if chains.len() >= max_chains {
        println!(
            "\n{}",
            "Output truncated; more dependency chains may exist.".dimmed()
        );
    }

    Ok(())
}
