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
    let _ = crate::secure_fs::ensure_private_dir(&path);
    path
}

pub fn data_dir() -> PathBuf {
    let path = xdg_data_home().join(APP_NAME);
    let _ = crate::secure_fs::ensure_private_dir(&path);
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
    let _ = crate::secure_fs::ensure_private_dir(&path);
    path
}

pub fn runtime_dir() -> PathBuf {
    let path = env::var_os("XDG_RUNTIME_DIR")
        .map(|runtime| PathBuf::from(runtime).join(APP_NAME))
        .unwrap_or_else(|| data_dir().join("runtime"));
    let _ = crate::secure_fs::ensure_private_dir(&path);
    path
}

pub fn cleanup_drag_temps() {
    let _ = crate::secure_fs::remove_files_with_prefix(&runtime_dir(), "drag-");
}

pub fn omarchy_theme_dir() -> PathBuf {
    home().join(".local/state/omarchy/current/theme")
}

pub fn omarchy_theme_name_path() -> PathBuf {
    home().join(".local/state/omarchy/current/theme.name")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn constants() {
        assert_eq!(APP_NAME, "omapaste");
        assert_eq!(APP_ID, "io.github.pkayokay.omapaste");
        assert_eq!(ISSUES_URL, "https://github.com/pkayokay/omapaste/issues");
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn xdg_paths_honor_env() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let old_config = env::var_os("XDG_CONFIG_HOME");
        let old_data = env::var_os("XDG_DATA_HOME");
        env::set_var("XDG_CONFIG_HOME", dir.path().join("cfg"));
        env::set_var("XDG_DATA_HOME", dir.path().join("data"));

        assert_eq!(config_path(), dir.path().join("cfg/omapaste/config.toml"));
        assert_eq!(db_path(), dir.path().join("data/omapaste/history.sqlite"));
        assert_eq!(images_dir(), dir.path().join("data/omapaste/images"));
        assert!(dir.path().join("cfg/omapaste").is_dir());
        assert!(dir.path().join("data/omapaste/images").is_dir());

        match old_config {
            Some(v) => env::set_var("XDG_CONFIG_HOME", v),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_data {
            Some(v) => env::set_var("XDG_DATA_HOME", v),
            None => env::remove_var("XDG_DATA_HOME"),
        }
    }

    #[test]
    fn runtime_dir_without_xdg_runtime_uses_private_data_runtime() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let old_runtime = env::var_os("XDG_RUNTIME_DIR");
        let old_data = env::var_os("XDG_DATA_HOME");
        let old_tmp = env::var_os("TMPDIR");
        env::set_var("XDG_DATA_HOME", dir.path().join("data"));
        env::remove_var("XDG_RUNTIME_DIR");
        env::remove_var("TMPDIR");

        assert_eq!(runtime_dir(), dir.path().join("data/omapaste/runtime"));
        #[cfg(unix)]
        assert_eq!(
            runtime_dir().metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );

        match old_runtime {
            Some(v) => env::set_var("XDG_RUNTIME_DIR", v),
            None => env::remove_var("XDG_RUNTIME_DIR"),
        }
        match old_data {
            Some(v) => env::set_var("XDG_DATA_HOME", v),
            None => env::remove_var("XDG_DATA_HOME"),
        }
        match old_tmp {
            Some(v) => env::set_var("TMPDIR", v),
            None => env::remove_var("TMPDIR"),
        }
    }

    #[test]
    fn cleanup_drag_temps_removes_only_drag_files() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let old_runtime = env::var_os("XDG_RUNTIME_DIR");
        env::set_var("XDG_RUNTIME_DIR", dir.path());

        let drag = runtime_dir().join("drag-leftover.png");
        crate::secure_fs::write_private_new_file(&drag, b"x").unwrap();
        let keep = runtime_dir().join("note.txt");
        crate::secure_fs::write_private_new_file(&keep, b"y").unwrap();

        cleanup_drag_temps();

        assert!(!drag.exists());
        assert!(keep.exists());

        match old_runtime {
            Some(v) => env::set_var("XDG_RUNTIME_DIR", v),
            None => env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    #[test]
    fn home_falls_back_to_home_env() {
        let _lock = crate::env_lock();
        let dir = tempfile::TempDir::new().unwrap();
        let old_home = env::var_os("HOME");
        let old_config = env::var_os("XDG_CONFIG_HOME");
        env::set_var("HOME", dir.path());
        env::remove_var("XDG_CONFIG_HOME");
        assert_eq!(xdg_config_home(), dir.path().join(".config"));
        assert_eq!(
            omarchy_theme_dir(),
            dir.path().join(".local/state/omarchy/current/theme")
        );
        match old_home {
            Some(v) => env::set_var("HOME", v),
            None => env::remove_var("HOME"),
        }
        match old_config {
            Some(v) => env::set_var("XDG_CONFIG_HOME", v),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
    }
}
