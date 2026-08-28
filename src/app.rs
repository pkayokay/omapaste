use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib, Application};

use crate::clipboard::ClipboardWatcher;
use crate::config::load_config;
use crate::paste::current_window;
use crate::paths::{cleanup_drag_temps, db_path, images_dir, APP_ID, VERSION};
use crate::store::Store;
use crate::theme::watch_paths;
use crate::ui::Overlay;

struct AppState {
    overlay: Rc<Overlay>,
    watcher: Rc<ClipboardWatcher>,
    store: Rc<Store>,
    max_items: i64,
    _hold: gio::ApplicationHoldGuard,
}

pub fn run(startup_command: &str) -> glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let state: Rc<RefCell<Option<AppState>>> = Rc::new(RefCell::new(None));
    let startup_command = startup_command.to_string();

    app.connect_startup({
        let state = state.clone();
        move |app| {
            let hold = app.hold();
            cleanup_drag_temps();
            let config = load_config(None);
            let db = db_path();
            let fresh = !db.exists();
            let store = match Store::open(&db) {
                Ok(s) => Rc::new(s),
                Err(err) => {
                    log::error!("failed to open history: {err}");
                    return;
                }
            };
            if fresh {
                if let Err(err) = store.seed(&images_dir()) {
                    log::warn!("failed to seed sample clips: {err}");
                }
            }
            if let Ok(removed) = store.prune(config.max_items, None) {
                unlink(&removed);
            }

            let ignore_slot: Rc<RefCell<Option<Rc<ClipboardWatcher>>>> =
                Rc::new(RefCell::new(None));
            let on_copy = {
                let slot = ignore_slot.clone();
                Rc::new(move |digest: &str| {
                    if let Some(w) = slot.borrow().as_ref() {
                        w.ignore_hash(digest, 1.5);
                    }
                }) as Rc<dyn Fn(&str)>
            };

            let overlay = Overlay::new(app, store.clone(), config.clone(), on_copy);
            let on_change = {
                let ov = overlay.clone();
                Rc::new(move |_clip| {
                    if ov.is_open() {
                        ov.refresh_rc(true);
                    }
                }) as Rc<dyn Fn(crate::store::Clip)>
            };
            let watcher = Rc::new(ClipboardWatcher::new(
                store.clone(),
                config.clone(),
                images_dir(),
                on_change,
            ));
            watcher.start();
            *ignore_slot.borrow_mut() = Some(watcher.clone());

            watch_theme(overlay.clone());
            let max_items = config.max_items;
            *state.borrow_mut() = Some(AppState {
                overlay: overlay.clone(),
                watcher,
                store: store.clone(),
                max_items,
                _hold: hold,
            });
            let state_prune = state.clone();
            glib::timeout_add_seconds_local(60, move || {
                if let Some(st) = state_prune.borrow().as_ref() {
                    if let Ok(removed) = st.store.prune(st.max_items, None) {
                        unlink(&removed);
                    }
                    if st.overlay.is_open() {
                        st.overlay.refresh_rc(true);
                    }
                }
                glib::ControlFlow::Continue
            });
        }
    });

    app.connect_command_line({
        let state = state.clone();
        move |_app, cmdline| {
            let args: Vec<String> = cmdline
                .arguments()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let cmd = args
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or(startup_command.as_str());
            handle(&state, cmd);
            glib::ExitCode::from(0)
        }
    });

    app.connect_shutdown({
        let state = state.clone();
        move |_| {
            if let Some(st) = state.borrow().as_ref() {
                st.watcher.stop();
            }
        }
    });

    app.run()
}

fn handle(state: &Rc<RefCell<Option<AppState>>>, command: &str) {
    if matches!(command, "quit" | "stop") {
        let app = state
            .borrow()
            .as_ref()
            .and_then(|st| st.overlay.window.application());
        if let Some(app) = app {
            app.quit();
        }
        return;
    }
    let st = state.borrow();
    let Some(st) = st.as_ref() else {
        return;
    };
    match command {
        "daemon" | "start" => log::info!("omapaste {VERSION} daemon ready"),
        "toggle" | "" => {
            if st.overlay.is_open() {
                st.overlay.hide_rc();
            } else {
                st.overlay.show_rc(current_window());
            }
        }
        "show" => st.overlay.show_rc(current_window()),
        "hide" => st.overlay.hide_rc(),
        other => log::warn!("unknown command: {other}"),
    }
}

fn watch_theme(overlay: Rc<Overlay>) {
    for path in watch_paths() {
        if !path.exists() {
            continue;
        }
        let file = gio::File::for_path(&path);
        match file.monitor_file(gio::FileMonitorFlags::empty(), gio::Cancellable::NONE) {
            Ok(monitor) => {
                let ov = overlay.clone();
                monitor.connect_changed(move |_, _, _, _| {
                    ov.reload_theme();
                });
                std::mem::forget(monitor);
            }
            Err(err) => log::debug!("theme monitor: {err}"),
        }
    }
}

fn unlink(paths: &[std::path::PathBuf]) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn unlink_removes_existing_and_ignores_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let present = dir.path().join("a.bin");
        let missing = dir.path().join("gone.bin");
        fs::write(&present, b"x").unwrap();
        unlink(&[present.clone(), missing]);
        assert!(!present.exists());
    }
}
