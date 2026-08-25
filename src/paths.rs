use std::env;
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.pkayokay.omapaste";
pub const APP_NAME: &str = "omapaste";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ISSUES_URL: &str = "https://github.com/pkayokay/omapaste/issues";

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(dirs_home)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn dirs_home() -> Option<PathBuf> {
    env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn xdg_config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
}

pub fn xdg_data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"))
}

pub fn config_dir() -> PathBuf {
    let path = xdg_config_home().join(APP_NAME);
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn data_dir() -> PathBuf {
    let path = xdg_data_home().join(APP_NAME);
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn db_path() -> PathBuf {
    data_dir().join("history.sqlite")
}

pub fn images_dir() -> PathBuf {
    let path = data_dir().join("images");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn omarchy_theme_dir() -> PathBuf {
    home().join(".local/state/omarchy/current/theme")
}

pub fn omarchy_theme_name_path() -> PathBuf {
    home().join(".local/state/omarchy/current/theme.name")
}
