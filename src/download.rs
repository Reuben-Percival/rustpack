use anyhow::{Result, Context, bail};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn download_file(url: &str, dest_path: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url)
        .context("Failed to download file")?;
    
    if !response.status().is_success() {
        bail!("Download failed with status: {}", response.status());
    }
    
    let mut file = File::create(dest_path)
        .context("Failed to create destination file")?;
    
    let content = response.bytes().context("Failed to read response")?;
    file.write_all(&content).context("Failed to write to file")?;
    
    Ok(())
}

pub fn download_database(server: &str, repo_name: &str, dest_dir: &Path) -> Result<()> {
    let db_filename = format!("{}.db", repo_name);
    let url = format!("{}/{}", server, db_filename);
    let dest = dest_dir.join(&db_filename);
    
    println!("  Downloading {} from {}", db_filename, server);
    download_file(&url, &dest)?;
    
    Ok(())
}

pub fn fetch_url_text(url: &str) -> Result<String> {
    let response = reqwest::blocking::get(url)?
        .error_for_status()?;
    
    let text = response.text()?;
    Ok(text)
}

pub fn fetch_url_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let response = reqwest::blocking::get(url)?
        .error_for_status()?;
    
    let data = response.json::<T>()?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fetch_url() {
        let result = fetch_url_text("https://httpbin.org/get");
        assert!(result.is_ok());
    }
}
