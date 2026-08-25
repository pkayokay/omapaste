from __future__ import annotations

import argparse
import sys

from omapaste import __version__

COMMANDS = ("daemon", "start", "toggle", "show", "hide", "quit", "stop")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="omapaste",
        description="Clipboard history for Omarchy. Toggle a bottom clip bar, pick a clip, paste it.",
    )
    parser.add_argument(
        "command",
        nargs="?",
        default="toggle",
        choices=COMMANDS,
        help="daemon keeps the watcher running. toggle shows or hides the bar (default).",
    )
    parser.add_argument("--version", action="version", version=f"omapaste {__version__}")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    import omapaste.gi_boot  # noqa: F401  — must load before GTK
    from omapaste.app import run

    return run(args.command)


if __name__ == "__main__":
    sys.exit(main())
