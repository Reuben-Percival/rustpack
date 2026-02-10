use std::process::Command;

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn get_arch() -> String {
    let output = Command::new("uname")
        .arg("-m")
        .output()
        .expect("Failed to get architecture");
    
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string()
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
    Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
