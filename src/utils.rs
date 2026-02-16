use std::env;
use std::path::PathBuf;

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn get_arch() -> String {
    env::consts::ARCH.to_string()
}

pub fn arch_variants(arch: &str) -> (String, String, String) {
    let base = if arch.starts_with("x86_64_v") {
        "x86_64"
    } else {
        arch
    };
    if base == "x86_64" {
        (base.to_string(), format!("{base}_v3"), format!("{base}_v4"))
    } else {
        (base.to_string(), base.to_string(), base.to_string())
    }
}

pub fn check_command_exists(command: &str) -> bool {
    let Some(path_env) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path_env).any(|dir| {
        let candidate: PathBuf = dir.join(command);
        candidate.is_file()
    })
}

pub fn confirm_action(message: &str) -> bool {
    use std::io::{self, Write};

    print!("{}", message);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    let response = input.trim().to_lowercase();

    // Default to yes if empty (like pacman)
    response.is_empty() || matches!(response.as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_arch() {
        let arch = get_arch();
        assert!(!arch.is_empty());
    }

    #[test]
    fn test_check_command_exists() {
        assert!(check_command_exists("ls"));
        assert!(!check_command_exists("nonexistent_command_xyz"));
    }
}
