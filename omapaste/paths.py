from __future__ import annotations

import os
from pathlib import Path


APP_ID = "io.github.pkayokay.omapaste"
APP_NAME = "omapaste"


def xdg_config_home() -> Path:
    return Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))


def xdg_data_home() -> Path:
    return Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))


def xdg_state_home() -> Path:
    return Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state"))


def xdg_runtime_dir() -> Path:
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if runtime:
        return Path(runtime)
    return Path("/tmp") / f"omapaste-{os.getuid()}"


def config_dir() -> Path:
    path = xdg_config_home() / APP_NAME
    path.mkdir(parents=True, exist_ok=True)
    return path


def data_dir() -> Path:
    path = xdg_data_home() / APP_NAME
    path.mkdir(parents=True, exist_ok=True)
    return path


def state_dir() -> Path:
    path = xdg_state_home() / APP_NAME
    path.mkdir(parents=True, exist_ok=True)
    return path


def config_path() -> Path:
    return config_dir() / "config.toml"


def db_path() -> Path:
    return data_dir() / "history.sqlite"


def images_dir() -> Path:
    path = data_dir() / "images"
    path.mkdir(parents=True, exist_ok=True)
    return path


def socket_path() -> Path:
    runtime = xdg_runtime_dir()
    runtime.mkdir(parents=True, exist_ok=True)
    return runtime / "omapaste.sock"


def omarchy_theme_dir() -> Path:
    return Path.home() / ".local/state/omarchy/current/theme"


def omarchy_theme_name_path() -> Path:
    return Path.home() / ".local/state/omarchy/current/theme.name"
