#[derive(Default, Clone)]
pub struct GlobalFlags {
    pub noconfirm: bool,
    pub needed: bool,
    pub overwrite: Vec<String>,
    pub asdeps: bool,
    pub asexplicit: bool,
    pub nodeps: u8,
    pub noscriptlet: bool,
    pub root_dir: Option<String>,
    pub db_path: Option<String>,
    pub cache_dir: Option<String>,
    pub test: bool,
    pub strict: bool,
    pub insecure_skip_signatures: bool,
    pub json: bool,
    pub compact: bool,
    pub verbose: bool,
    pub doctor_fix: bool,
    pub parallel_downloads: Option<u32>,
    pub disable_download_timeout: bool,
}

#[derive(Default, Clone)]
pub struct RemoveFlags {
    pub recursive: bool,
    pub nosave: bool,
}
