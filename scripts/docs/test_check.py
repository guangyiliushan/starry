"""Tests for the Starry documentation checker."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check import (
    check_links,
    check_parity,
    check_terms,
    load_manifest,
    validate_manifest,
)


def write(path: Path, text: str = "") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def manifest(pages: list[dict]) -> dict:
    return {
        "schema": 1,
        "locales": ["en", "zh-cn"],
        "defaultLocale": "en",
        "versions": [],
        "pages": pages,
    }


class ManifestTests(unittest.TestCase):
    def test_manifest_rejects_missing_page(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / "site/data/manifest.json", "{}")
            write(root / "docs/en/index.md")
            errors = validate_manifest(load_manifest(root / "site/data/manifest.json"), root)
            self.assertIn("manifest.schema must be 1", errors)

    def test_manifest_rejects_nonempty_versions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([])
            data["versions"] = ["1"]
            errors = validate_manifest(data, root)
            self.assertIn("manifest.versions must remain an empty list", errors)

    def test_parity_detects_missing_zh_cn(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / "docs/en/index.md")
            errors = check_parity(root, manifest([{"path": "index"}]))
            self.assertEqual(errors, ["zh-cn/index.md is missing"])

    def test_terms_detects_internal_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / "docs/en/index.md", "See crates/ast.")
            write(root / "docs/zh-cn/index.md")
            errors = check_terms(root, manifest([{"path": "index"}]))
            self.assertTrue(any("crates/" in error for error in errors))

    def test_links_detects_missing_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / "docs/en/index.md", "[missing](missing.md)")
            write(root / "docs/zh-cn/index.md")
            errors = check_links(root, manifest([{"path": "index"}]))
            self.assertTrue(any("missing link" in error for error in errors))


if __name__ == "__main__":
    unittest.main()