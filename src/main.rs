mod config;
mod install;
mod search;
mod utils;
mod alpm_ops;
mod cli;

use anyhow::Result;
use colored::Colorize;
use std::env;
use crate::cli::{GlobalFlags, RemoveFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Sync,
    Query,
    Remove,
    Upgrade,
    Help,
}

#[derive(Default)]
struct SyncFlags {
    refresh: bool,
    upgrade: bool,
    search: bool,
    info: bool,
    clean_cache: u8,
}

#[derive(Default)]
struct QueryFlags {
    info: bool,
    search: bool,
    list_files: bool,
    manual: bool,
    owns: bool,
}

struct ParsedArgs {
    op: Operation,
    sync: SyncFlags,
    query: QueryFlags,
    remove: RemoveFlags,
    targets: Vec<String>,
    global: GlobalFlags,
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // Need at least the program name and one argument
    if args.len() < 2 {
        print_usage();
        std::process::exit(0);
    }

    if args.iter().any(|a| a == "--aur" || a == "--paru") {
        if utils::is_root() {
            eprintln!("error: --aur/--paru must be run as a regular user (do not use sudo)");
            std::process::exit(1);
        }
        if !utils::check_command_exists("paru") {
            eprintln!("error: paru not found in PATH (install paru or run without --aur)");
            std::process::exit(1);
        }
        let filtered: Vec<String> = args
            .into_iter()
            .skip(1)
            .filter(|a| a != "--aur" && a != "--paru")
            .collect();
        let status = std::process::Command::new("paru")
            .args(filtered)
            .status()
            .expect("failed to execute paru");
        std::process::exit(status.code().unwrap_or(1));
    }
    
    let parsed = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{}", message);
            eprintln!("Try 'rustpack --help' for more information.");
            std::process::exit(1);
        }
    };
    
    match parsed.op {
        Operation::Sync => handle_sync(&parsed)?,
        Operation::Query => handle_query(&parsed)?,
        Operation::Remove => handle_remove(&parsed)?,
        Operation::Upgrade => handle_upgrade(&parsed)?,
        Operation::Help => print_usage(),
    }
    
    Ok(())
}

fn set_operation(op: &mut Option<Operation>, new_op: Operation) -> std::result::Result<(), String> {
    if let Some(existing) = op {
        if *existing != new_op {
            return Err("error: only one operation may be used at a time".to_string());
        }
    } else {
        *op = Some(new_op);
    }
    Ok(())
}

fn parse_args(args: &[String]) -> std::result::Result<ParsedArgs, String> {
    let mut op: Option<Operation> = None;
    let mut flag_chars: Vec<char> = Vec::new();
    let mut targets: Vec<String> = Vec::new();
    let mut in_options = true;
    let mut global = GlobalFlags::default();
    let mut i = 1;
    
    while i < args.len() {
        let arg = &args[i];
        if in_options && (arg == "-h" || arg == "--help") {
            return Ok(ParsedArgs {
                op: Operation::Help,
                sync: SyncFlags::default(),
                query: QueryFlags::default(),
                remove: RemoveFlags::default(),
                targets: Vec::new(),
                global: GlobalFlags::default(),
            });
        }
        
        if in_options && arg == "--" {
            in_options = false;
            i += 1;
            continue;
        }
        
        if in_options && arg.starts_with("--") {
            let (key, value_opt) = if let Some((k, v)) = arg.split_once('=') {
                (k, Some(v.to_string()))
            } else {
                (arg.as_str(), None)
            };
            
            match key {
                "--test" | "--dry-run" => global.test = true,
                "--noconfirm" => global.noconfirm = true,
                "--needed" => global.needed = true,
                "--nodeps" => global.nodeps = global.nodeps.saturating_add(1),
                "--noscriptlet" => global.noscriptlet = true,
                "--asdeps" => global.asdeps = true,
                "--asexplicit" => global.asexplicit = true,
                "--overwrite" => {
                    let value = value_opt.or_else(|| {
                        if i + 1 < args.len() {
                            i += 1;
                            Some(args[i].to_string())
                        } else {
                            None
                        }
                    });
                    let value = value.ok_or_else(|| "error: --overwrite requires a value".to_string())?;
                    global.overwrite.push(value);
                }
                "--root" => {
                    let value = value_opt.or_else(|| {
                        if i + 1 < args.len() {
                            i += 1;
                            Some(args[i].to_string())
                        } else {
                            None
                        }
                    });
                    global.root_dir = Some(value.ok_or_else(|| "error: --root requires a value".to_string())?);
                }
                "--dbpath" => {
                    let value = value_opt.or_else(|| {
                        if i + 1 < args.len() {
                            i += 1;
                            Some(args[i].to_string())
                        } else {
                            None
                        }
                    });
                    global.db_path = Some(value.ok_or_else(|| "error: --dbpath requires a value".to_string())?);
                }
                "--cachedir" => {
                    let value = value_opt.or_else(|| {
                        if i + 1 < args.len() {
                            i += 1;
                            Some(args[i].to_string())
                        } else {
                            None
                        }
                    });
                    global.cache_dir = Some(value.ok_or_else(|| "error: --cachedir requires a value".to_string())?);
                }
                _ => return Err(format!("error: invalid option '{}'", arg)),
            }
            i += 1;
            continue;
        }
        
        if in_options && arg.starts_with('-') && arg.len() > 1 {
            for ch in arg[1..].chars() {
                match ch {
                    'S' => set_operation(&mut op, Operation::Sync)?,
                    'Q' => set_operation(&mut op, Operation::Query)?,
                    'R' => set_operation(&mut op, Operation::Remove)?,
                    'U' => set_operation(&mut op, Operation::Upgrade)?,
                    _ => flag_chars.push(ch),
                }
            }
            i += 1;
            continue;
        }
        
        targets.push(arg.to_string());
        i += 1;
    }
    
    let op = op.ok_or_else(|| "error: no operation specified (use -h for help)".to_string())?;
    let mut parsed = ParsedArgs {
        op,
        sync: SyncFlags::default(),
        query: QueryFlags::default(),
        remove: RemoveFlags::default(),
        targets,
        global,
    };
    
    match op {
        Operation::Sync => {
            for ch in flag_chars {
                match ch {
                    'y' => parsed.sync.refresh = true,
                    'u' => parsed.sync.upgrade = true,
                    's' => parsed.sync.search = true,
                    'i' => parsed.sync.info = true,
                    'd' => parsed.global.nodeps = parsed.global.nodeps.saturating_add(1),
                    'c' => parsed.sync.clean_cache = parsed.sync.clean_cache.saturating_add(1),
                    _ => return Err(format!("error: invalid option '-{}' for -S", ch)),
                }
            }
            
            if parsed.sync.search && parsed.sync.info {
                return Err("error: only one of -s or -i can be used with -S".to_string());
            }
            
            if (parsed.sync.search || parsed.sync.info) && (parsed.sync.refresh || parsed.sync.upgrade) {
                return Err("error: -s/-i cannot be combined with -y/-u".to_string());
            }
            
            if (parsed.sync.search || parsed.sync.info) && parsed.targets.is_empty() {
                return Err("error: no targets specified (use -h for help)".to_string());
            }
            
            if !parsed.sync.search
                && !parsed.sync.info
                && parsed.targets.is_empty()
                && !parsed.sync.refresh
                && !parsed.sync.upgrade
                && parsed.sync.clean_cache == 0
            {
                return Err("error: no targets specified (use -h for help)".to_string());
            }
            
            if parsed.sync.clean_cache > 0 {
                if parsed.sync.search
                    || parsed.sync.info
                    || parsed.sync.refresh
                    || parsed.sync.upgrade
                    || !parsed.targets.is_empty()
                {
                    return Err("error: -Sc/-Scc cannot be combined with other -S options".to_string());
                }
            }
            
            if parsed.global.asdeps && parsed.global.asexplicit {
                return Err("error: --asdeps and --asexplicit cannot be used together".to_string());
            }
        }
        Operation::Query => {
            for ch in flag_chars {
                match ch {
                    'i' => parsed.query.info = true,
                    's' => parsed.query.search = true,
                    'l' => parsed.query.list_files = true,
                    'm' => parsed.query.manual = true,
                    'o' => parsed.query.owns = true,
                    _ => return Err(format!("error: invalid option '-{}' for -Q", ch)),
                }
            }
            
            let mut option_count = 0;
            if parsed.query.info {
                option_count += 1;
            }
            if parsed.query.search {
                option_count += 1;
            }
            if parsed.query.list_files {
                option_count += 1;
            }
            if parsed.query.manual {
                option_count += 1;
            }
            if parsed.query.owns {
                option_count += 1;
            }
            
            if option_count > 1 {
                return Err("error: only one of -i, -s, -l, -m, or -o can be used with -Q".to_string());
            }
            
            if (parsed.query.info || parsed.query.search || parsed.query.list_files || parsed.query.owns)
                && parsed.targets.is_empty()
            {
                return Err("error: no targets specified (use -h for help)".to_string());
            }
            
            if parsed.query.manual && !parsed.targets.is_empty() {
                return Err("error: -Qm does not take targets".to_string());
            }
        }
        Operation::Remove => {
            for ch in flag_chars {
                match ch {
                    's' => parsed.remove.recursive = true,
                    'n' => parsed.remove.nosave = true,
                    'd' => parsed.global.nodeps = parsed.global.nodeps.saturating_add(1),
                    _ => return Err(format!("error: invalid option '-{}' for -R", ch)),
                }
            }
            
            if parsed.targets.is_empty() {
                return Err("error: no targets specified (use -h for help)".to_string());
            }
            
            if parsed.global.asdeps || parsed.global.asexplicit || parsed.global.needed || parsed.global.noscriptlet {
                return Err("error: invalid options for -R".to_string());
            }
        }
        Operation::Upgrade => {
            for ch in flag_chars {
                match ch {
                    'd' => parsed.global.nodeps = parsed.global.nodeps.saturating_add(1),
                    _ => return Err(format!("error: invalid option '-{}' for -U", ch)),
                }
            }
            
            if parsed.targets.is_empty() {
                return Err("error: no targets specified (use -h for help)".to_string());
            }
        }
        Operation::Help => {}
    }
    
    if parsed.op != Operation::Sync {
        if parsed.global.needed || parsed.global.asdeps || parsed.global.asexplicit || parsed.global.noscriptlet {
            return Err("error: --needed/--asdeps/--asexplicit/--noscriptlet only apply to -S".to_string());
        }
        if !parsed.global.overwrite.is_empty() {
            return Err("error: --overwrite only applies to -S".to_string());
        }
    }
    
    if parsed.op == Operation::Query && parsed.global.nodeps > 0 {
        return Err("error: --nodeps only applies to -S/-R/-U".to_string());
    }
    
    Ok(parsed)
}

fn handle_sync(parsed: &ParsedArgs) -> Result<()> {
    let flags = &parsed.sync;
    
    // Check root for install/upgrade/sync
    if !flags.search && !flags.info && !utils::is_root() {
        eprintln!("{}", "error: you cannot perform this operation unless you are root.".red());
        std::process::exit(1);
    }
    
    if flags.search {
        search_packages(&parsed.global, &parsed.targets)?;
        return Ok(());
    }
    
    if flags.info {
        for pkg in &parsed.targets {
            show_sync_info(&parsed.global, pkg)?;
        }
        return Ok(());
    }
    
    if flags.clean_cache > 0 {
        install::clean_cache(&parsed.global, flags.clean_cache)?;
        return Ok(());
    }
    
    let refresh = flags.refresh;
    let upgrade = flags.upgrade;
    if refresh || upgrade || parsed.targets.is_empty() {
        install::sync_install(
            &parsed.global,
            refresh,
            upgrade,
            parsed.targets.as_slice(),
        )?;
        return Ok(());
    }
    
    install_packages(parsed.targets.clone(), &parsed.global)?;
    
    Ok(())
}

fn handle_query(parsed: &ParsedArgs) -> Result<()> {
    let flags = &parsed.query;
    
    if flags.info {
        for pkg in &parsed.targets {
            search::show_package_info(&parsed.global, pkg)?;
        }
        return Ok(());
    }
    
    if flags.search {
        query_search_packages(&parsed.global, &parsed.targets)?;
        return Ok(());
    }
    
    if flags.list_files {
        search::list_package_files(&parsed.global, &parsed.targets)?;
        return Ok(());
    }
    
    if flags.manual {
        search::list_manual_packages(&parsed.global)?;
        return Ok(());
    }
    
    if flags.owns {
        search::query_owns(&parsed.global, &parsed.targets)?;
        return Ok(());
    }
    
    if parsed.targets.is_empty() {
        query_list_packages(&parsed.global)?;
    } else {
        search::query_packages(&parsed.global, &parsed.targets)?;
    }
    
    Ok(())
}

fn handle_remove(parsed: &ParsedArgs) -> Result<()> {
    if !utils::is_root() {
        eprintln!("{}", "error: you cannot perform this operation unless you are root.".red());
        std::process::exit(1);
    }
    
    remove_packages(parsed.targets.clone(), &parsed.remove, &parsed.global)?;
    
    Ok(())
}

fn handle_upgrade(parsed: &ParsedArgs) -> Result<()> {
    if !utils::is_root() {
        eprintln!("{}", "error: you cannot perform this operation unless you are root.".red());
        std::process::exit(1);
    }
    
    install::install_local(&parsed.global, &parsed.targets)?;
    Ok(())
}

fn print_usage() {
    println!("rustpack - A Rust-based package manager for Arch Linux");
    println!();
    println!("Usage: rustpack <operation> [options] [targets]");
    println!();
    println!("Operations:");
    println!("  -S [y|u|s|i]    Sync/upgrade, search, or info");
    println!("  -Q [i|s|l|m|o]  Query installed packages");
    println!("  -R [s|n]        Remove packages");
    println!("  -U <pkgfile>    Install local package file");
    println!();
    println!("Examples:");
    println!("  rustpack -Ss firefox      Search for firefox");
    println!("  rustpack -S firefox       Install firefox");
    println!("  rustpack -Syu             Full system upgrade");
    println!("  rustpack -Q               List installed packages");
    println!("  rustpack -Ql bash         List files for bash");
    println!("  rustpack -Qm              List foreign packages");
    println!("  rustpack -Qo /usr/bin/vi  Find owning package");
    println!("  rustpack -R firefox       Remove firefox");
    println!("  rustpack -Rns firefox     Remove firefox and its unused deps");
    println!("  rustpack -U ./pkg.pkg.tar.zst  Install a local package file");
    println!("  rustpack -Sc              Clean unused cache");
    println!();
    println!("Notes:");
    println!("  Use '--' to stop option parsing, e.g. rustpack -S -- -weirdpkg");
    println!("  Use '--test' to simulate changes without committing");
    println!("  Common options: --noconfirm --needed --overwrite --asdeps --asexplicit");
    println!("                  --root --dbpath --cachedir");
    println!("  Dependency options: -d/-dd (--nodeps), --noscriptlet");
    println!("  Cache clean: -Sc (unused) or -Scc (all)");
}

fn install_packages(packages: Vec<String>, global: &GlobalFlags) -> Result<()> {
    install::install_packages(&packages, global)?;
    
    Ok(())
}

fn search_packages(global: &GlobalFlags, queries: &[String]) -> Result<()> {
    search::search_repos(global, queries)?;
    Ok(())
}

fn show_sync_info(global: &GlobalFlags, package: &str) -> Result<()> {
    search::show_sync_package_info(global, package)?;
    Ok(())
}

fn query_list_packages(global: &GlobalFlags) -> Result<()> {
    search::list_installed(global)?;
    Ok(())
}

fn query_search_packages(global: &GlobalFlags, queries: &[String]) -> Result<()> {
    search::search_installed(global, queries)?;
    Ok(())
}

fn remove_packages(packages: Vec<String>, remove: &RemoveFlags, global: &GlobalFlags) -> Result<()> {
    install::remove_packages(&packages, remove, global)?;
    
    Ok(())
}
