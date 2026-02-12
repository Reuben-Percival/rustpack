use anyhow::Result;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::GlobalFlags;

#[derive(Debug, Clone)]
struct Entry {
    id: String,
    ts: u64,
    op: String,
    status: String,
    targets: String,
    summary: String,
}

fn escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('|', "\\p")
        .replace('\n', "\\n")
}

fn unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    '\\' => out.push('\\'),
                    'p' => out.push('|'),
                    'n' => out.push('\n'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn history_dir(global: &GlobalFlags) -> PathBuf {
    if let Some(root) = global.root_dir.as_ref() {
        if root == "/" {
            return Path::new("/var/log/rustpack").to_path_buf();
        }
        return Path::new(root).join("var/log/rustpack");
    }
    Path::new("/var/log/rustpack").to_path_buf()
}

fn history_file(global: &GlobalFlags) -> PathBuf {
    history_dir(global).join("history.log")
}

fn parse_entry(line: &str) -> Option<Entry> {
    let parts: Vec<&str> = line.splitn(6, '|').collect();
    if parts.len() != 6 {
        return None;
    }
    let ts = parts[1].parse::<u64>().ok()?;
    Some(Entry {
        id: unescape(parts[0]),
        ts,
        op: unescape(parts[2]),
        status: unescape(parts[3]),
        targets: unescape(parts[4]),
        summary: unescape(parts[5]),
    })
}

fn read_entries(global: &GlobalFlags) -> Result<Vec<Entry>> {
    let file = history_file(global);
    if !file.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(file)?;
    Ok(content.lines().filter_map(parse_entry).collect())
}

pub fn record(
    global: &GlobalFlags,
    operation: &str,
    status: &str,
    targets: &[String],
    summary: &str,
) -> Result<()> {
    let dir = history_dir(global);
    fs::create_dir_all(&dir)?;
    let file = history_file(global);
    let now = now_secs();
    let id = format!("{}-{}", now, process::id());
    let target_text = if targets.is_empty() {
        "-".to_string()
    } else {
        targets.join(" ")
    };
    let line = format!(
        "{}|{}|{}|{}|{}|{}\n",
        escape(&id),
        now,
        escape(operation),
        escape(status),
        escape(&target_text),
        escape(summary)
    );
    let mut f = OpenOptions::new().create(true).append(true).open(file)?;
    f.write_all(line.as_bytes())?;
    Ok(())
}

pub fn show(global: &GlobalFlags, args: &[String]) -> Result<()> {
    let entries = read_entries(global)?;
    if entries.is_empty() {
        println!("No history entries found.");
        return Ok(());
    }
    if args.is_empty() {
        print_list(&entries, 20);
        return Ok(());
    }
    if args[0] == "show" {
        if args.len() < 2 {
            println!("usage: rustpack history show <id>");
            return Ok(());
        }
        let id = &args[1];
        if let Some(entry) = entries.iter().find(|e| &e.id == id) {
            print_entry(entry);
        } else {
            println!("history entry not found: {}", id);
        }
        return Ok(());
    }
    if let Ok(limit) = args[0].parse::<usize>() {
        print_list(&entries, limit.max(1));
        return Ok(());
    }
    println!("usage:");
    println!("  rustpack history");
    println!("  rustpack history <limit>");
    println!("  rustpack history show <id>");
    Ok(())
}

fn print_list(entries: &[Entry], limit: usize) {
    let start = entries.len().saturating_sub(limit);
    println!("Recent rustpack history:");
    for e in entries[start..].iter().rev() {
        println!(
            "{}  ts={}  op={}  status={}  targets={}",
            e.id, e.ts, e.op, e.status, e.targets
        );
    }
}

fn print_entry(entry: &Entry) {
    println!("id      : {}", entry.id);
    println!("ts      : {}", entry.ts);
    println!("op      : {}", entry.op);
    println!("status  : {}", entry.status);
    println!("targets : {}", entry.targets);
    println!("summary : {}", entry.summary);
}
