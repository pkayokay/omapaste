from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from omapaste.store import Store, content_hash, keep_until_from, next_keep


class StoreTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.store = Store(Path(self.tmp.name) / "history.sqlite")

    def tearDown(self) -> None:
        self.store.close()
        self.tmp.cleanup()

    def _add(self, text: str, keep: str = "1d", now: int = 1_000):
        payload = text.encode()
        clip = self.store.add(
            kind="text",
            mime="text/plain",
            payload=payload,
            text=text,
            preview=text,
            image_path=None,
            keep_preset=keep,
            max_items=50,
            now=now,
        )
        assert clip is not None
        return clip

    def test_dedup_moves_to_front(self) -> None:
        first = self._add("hello", now=10)
        second = self._add("world", now=20)
        again = self._add("hello", now=30)
        self.assertEqual(first.id, again.id)
        clips = self.store.list(now=31)
        self.assertEqual([c.text for c in clips], ["hello", "world"])
        self.assertEqual(second.text, "world")

    def test_expired_clips_are_hidden_and_pruned(self) -> None:
        self._add("short", keep="1h", now=100)
        self._add("kept", keep="forever", now=100)
        visible = self.store.list(now=100 + 60 * 60 + 1)
        self.assertEqual([c.text for c in visible], ["kept"])
        self.store.prune(max_items=50, now=100 + 60 * 60 + 1)
        self.assertEqual(self.store.count(), 1)

    def test_set_keep_forever(self) -> None:
        clip = self._add("pin me", keep="1h", now=50)
        updated = self.store.set_keep(clip.id, "forever", now=80)
        self.assertEqual(updated.keep_preset, "forever")
        self.assertIsNone(updated.keep_until)

    def test_max_items_keeps_forever_last(self) -> None:
        self._add("a", keep="1d", now=1)
        self._add("b", keep="forever", now=2)
        self._add("c", keep="1d", now=3)
        self.store.prune(max_items=2, now=4)
        texts = {c.text for c in self.store.list(now=4)}
        self.assertIn("b", texts)
        self.assertEqual(len(texts), 2)

    def test_search_filters_preview(self) -> None:
        self._add("https://omarchy.org", now=1)
        self._add("invoice-42", now=2)
        hits = self.store.list("omarchy", now=3)
        self.assertEqual([c.text for c in hits], ["https://omarchy.org"])

    def test_hash_stable(self) -> None:
        self.assertEqual(
            content_hash("text", "text/plain", b"abc"),
            content_hash("text", "text/plain", b"abc"),
        )
        self.assertNotEqual(
            content_hash("text", "text/plain", b"abc"),
            content_hash("text", "text/plain", b"abcd"),
        )

    def test_keep_cycle(self) -> None:
        self.assertEqual(next_keep("1h").key, "1d")
        self.assertEqual(next_keep("1d").key, "7d")
        self.assertEqual(next_keep("7d").key, "forever")
        self.assertEqual(next_keep("forever").key, "1h")

    def test_keep_until_forever_is_none(self) -> None:
        self.assertIsNone(keep_until_from("forever", now=10))
        self.assertEqual(keep_until_from("1h", now=10), 10 + 3600)


if __name__ == "__main__":
    unittest.main()
