from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from omapaste.config import load_config


class ConfigTests(unittest.TestCase):
    def test_missing_file_writes_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "config.toml"
            cfg = load_config(path)
            self.assertTrue(path.exists())
            self.assertEqual(cfg.default_keep, "1d")
            self.assertEqual(cfg.paste_keys, "auto")

    def test_invalid_keep_falls_back(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "config.toml"
            path.write_text('default_keep = "nope"\npaste_keys = "laser"\n')
            cfg = load_config(path)
            self.assertEqual(cfg.default_keep, "1d")
            self.assertEqual(cfg.paste_keys, "auto")

    def test_custom_values(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "config.toml"
            path.write_text(
                'default_keep = "forever"\nmax_items = 12\nignore_secrets = false\npaste_keys = "shift-insert"\n'
            )
            cfg = load_config(path)
            self.assertEqual(cfg.default_keep, "forever")
            self.assertEqual(cfg.max_items, 12)
            self.assertFalse(cfg.ignore_secrets)
            self.assertEqual(cfg.paste_keys, "shift-insert")
            self.assertIsNone(cfg.keep_seconds())


if __name__ == "__main__":
    unittest.main()
