use anyhow::{Result, Context};
use std::fs;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct PacmanConfig {
    pub root_dir: String,
    pub db_path: String,
    pub cache_dir: String,
    pub hook_dirs: Vec<String>,
    pub gpg_dir: Option<String>,
    pub log_file: Option<String>,
    pub use_syslog: bool,
    pub check_space: bool,
    pub architectures: Vec<String>,
    pub sig_level: Option<String>,
    pub local_file_sig_level: Option<String>,
    pub remote_file_sig_level: Option<String>,
    pub repositories: Vec<Repository>,
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub name: String,
    pub servers: Vec<String>,
    pub sig_level: String,
}

impl Default for PacmanConfig {
    fn default() -> Self {
        PacmanConfig {
            root_dir: "/".to_string(),
            db_path: "/var/lib/pacman".to_string(),
            cache_dir: "/var/cache/pacman/pkg".to_string(),
            hook_dirs: Vec::new(),
            gpg_dir: None,
            log_file: None,
            use_syslog: false,
            check_space: false,
            architectures: Vec::new(),
            sig_level: None,
            local_file_sig_level: None,
            remote_file_sig_level: None,
            repositories: Vec::new(),
        }
    }
}

pub fn parse_pacman_config(path: &str) -> Result<PacmanConfig> {
    let content = fs::read_to_string(path)
        .context(format!("Failed to read {}", path))?;
    
    let mut config = PacmanConfig::default();
    let mut current_repo: Option<Repository> = None;
    let mut in_options = false;
    
    let repo_regex = Regex::new(r"^\[([^\]]+)\]").unwrap();
    let option_regex = Regex::new(r"^(\w+)\s*=\s*(.+)").unwrap();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Check for repository section
        if let Some(caps) = repo_regex.captures(line) {
            let section_name = caps.get(1).unwrap().as_str();
            
            // Save previous repository
            if let Some(repo) = current_repo.take() {
                if !repo.servers.is_empty() {
                    config.repositories.push(repo);
                }
            }
            
            // Track [options] section
            in_options = section_name == "options";
            if section_name != "options" {
                current_repo = Some(Repository {
                    name: section_name.to_string(),
                    servers: Vec::new(),
                    sig_level: "Required DatabaseOptional".to_string(),
                });
            }
            continue;
        }
        
        if in_options {
            // Boolean-style options
            match line {
                "CheckSpace" => {
                    config.check_space = true;
                    continue;
                }
                "UseSyslog" => {
                    config.use_syslog = true;
                    continue;
                }
                _ => {}
            }
        }
        
        // Parse options
        if let Some(caps) = option_regex.captures(line) {
            let key = caps.get(1).unwrap().as_str();
            let value = caps.get(2).unwrap().as_str();
            
            match key {
                "RootDir" => config.root_dir = value.to_string(),
                "DBPath" => config.db_path = value.to_string(),
                "CacheDir" => config.cache_dir = value.to_string(),
                "HookDir" if in_options => config.hook_dirs.push(value.to_string()),
                "GPGDir" if in_options => config.gpg_dir = Some(value.to_string()),
                "LogFile" if in_options => config.log_file = Some(value.to_string()),
                "Architecture" if in_options => config.architectures.push(value.to_string()),
                "SigLevel" if in_options => config.sig_level = Some(value.to_string()),
                "LocalFileSigLevel" if in_options => {
                    config.local_file_sig_level = Some(value.to_string())
                }
                "RemoteFileSigLevel" if in_options => {
                    config.remote_file_sig_level = Some(value.to_string())
                }
                "Server" => {
                    if let Some(ref mut repo) = current_repo {
                        repo.servers.push(value.to_string());
                    }
                }
                "Include" => {
                    // Parse included mirrorlist file
                    if let Some(ref mut repo) = current_repo {
                        if let Ok(servers) = parse_mirrorlist(value) {
                            repo.servers.extend(servers);
                        }
                    }
                }
                "SigLevel" => {
                    if let Some(ref mut repo) = current_repo {
                        repo.sig_level = value.to_string();
                    }
                }
                _ => {}
            }
        }
    }
    
    // Save last repository
    if let Some(repo) = current_repo {
        if !repo.servers.is_empty() {
            config.repositories.push(repo);
        }
    }
    
    Ok(config)
}

fn parse_mirrorlist(path: &str) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)?;
    let mut servers = Vec::new();
    
    let server_regex = Regex::new(r"^\s*Server\s*=\s*(.+)").unwrap();
    
    for line in content.lines() {
        if let Some(caps) = server_regex.captures(line) {
            let server = caps.get(1).unwrap().as_str().to_string();
            servers.push(server);
        }
    }
    
    Ok(servers)
}

pub fn expand_server_url(server: &str, repo_name: &str, arch: &str, arch_v3: &str, arch_v4: &str) -> String {
    server
        .replace("$repo", repo_name)
        .replace("$arch_v3", arch_v3)
        .replace("$arch_v4", arch_v4)
        .replace("$arch", arch)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_expand_server_url() {
        let url = "https://mirror.example.com/$repo/os/$arch";
        let expanded = expand_server_url(url, "core", "x86_64", "x86_64_v3", "x86_64_v4");
        assert_eq!(expanded, "https://mirror.example.com/core/os/x86_64");
    }
}
