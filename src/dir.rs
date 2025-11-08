use std::path::Path;
use crate::XCapError;
use directories::ProjectDirs;
use std::sync::LazyLock;

static PROJECT_DIR: LazyLock<ProjectDirs> = LazyLock::new(|| {
    let dir = directories::ProjectDirs::from("com", "xcap", "xcap")
        .ok_or_else(|| XCapError::new("failed to get project directory"))
        .expect("failed to get project directory");
    
    dir
});

pub fn project_dir() -> &'static ProjectDirs {
    &PROJECT_DIR
}

pub fn config_dir() -> &'static Path {
    PROJECT_DIR.config_dir()
}

pub fn cache_dir() -> &'static Path {
    PROJECT_DIR.cache_dir()
}

pub fn data_dir() -> &'static Path {
    PROJECT_DIR.data_dir()
}