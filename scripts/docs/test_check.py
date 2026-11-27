"""Tests for the Starry documentation checker."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from subprocess import CompletedProcess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check import (
    check_llms,
    check_links,
    check_parity,
    check_spec,
    check_terms,
    extract_fences,
    load_manifest,
    run_examples,
    validate_example_pairs,
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
            write(root / "docs/en/_index.md")
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
            write(root / "docs/en/_index.md")
            errors = check_parity(root, manifest([{"path": "index"}]))
            self.assertEqual(errors, ["zh-cn/_index.md is missing"])

    def test_terms_detects_internal_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / "docs/en/_index.md", "See crates/ast.")
            write(root / "docs/zh-cn/_index.md")
            errors = check_terms(root, manifest([{"path": "index"}]))
            self.assertTrue(any("crates/" in error for error in errors))

    def test_links_detects_missing_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write(root / "docs/en/_index.md", "[missing](missing.md)")
            write(root / "docs/zh-cn/_index.md")
            errors = check_links(root, manifest([{"path": "index"}]))
            self.assertTrue(any("missing link" in error for error in errors))


class SpecTests(unittest.TestCase):
    def test_spec_detects_missing_include(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([{"path": "spec/grammar", "type": "spec", "roles": ["lookup"]}])
            write(root / "docs/syntax/starry.ebnf", "Grammar")
            write(root / "docs/en/spec/grammar.md", "missing")
            write(root / "docs/zh-cn/spec/grammar.md", "missing")
            errors = check_spec(root, data, syntax_check=False)
            self.assertTrue(any("missing required include" in error for error in errors))


class LlmsTests(unittest.TestCase):
    def test_llms_contains_every_public_page(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([{"path": "index"}])
            write(root / "site/public/llms.txt", "wrong")
            errors = check_llms(root, data)
            self.assertTrue(any("has 0 entries for en/index" in error for error in errors))


class ExampleTests(unittest.TestCase):
    def test_extract_fences_pairs_run_with_expected_output(self) -> None:
        text = "```starry run\nfun main(): int {\n    print(\"ok\")\n    return 0\n}\n```\n\n```text\nok\n```\n"
        fences = extract_fences(text)
        errors = validate_example_pairs("index.md", fences)
        self.assertEqual(errors, [])
        self.assertEqual([(fence.info, fence.code) for fence in fences], [
            ("starry run", "fun main(): int {\n    print(\"ok\")\n    return 0\n}"),
            ("text", "ok"),
        ])

    def test_example_parity_detects_code_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([{"path": "index"}])
            write(root / "docs/en/_index.md", "```starry\nval x = 1\n```")
            write(root / "docs/zh-cn/_index.md", "```starry\nval x = 2\n```")
            errors = check_parity(root, data)
            self.assertTrue(any("code fence" in error for error in errors))

    def test_examples_resolve_compiler_relative_to_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([{"path": "start/tutorial"}])
            write(root / "docs/en/start/tutorial.md", "```starry check\nfun main(): int {\n    return 0\n}\n```\n")
            write(root / "docs/zh-cn/start/tutorial.md", "```starry check\nfun main(): int {\n    return 0\n}\n```\n")
            write(root / "target/debug/starryc")
            commands = []

            def runner(command, cwd, text_input=None):
                commands.append(command)
                return CompletedProcess(command, 0, "", "")

            errors = run_examples(root, data, "target/debug/starryc", runner=runner)
            self.assertEqual(errors, [])
            self.assertEqual(len(commands), 1)
            self.assertTrue(Path(commands[0][0]).is_absolute())
            self.assertTrue(Path(commands[0][0]).is_file())

    def test_positive_example_must_match_stdout(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([{"path": "index"}])
            source = "```starry run\nfun main(): int {\n    print(\"ok\")\n    return 0\n}\n```\n\n```text\nnope\n```\n"
            write(root / "docs/en/_index.md", source)
            write(root / "docs/zh-cn/_index.md", source)

            def runner(command, cwd, text_input=None):
                if len(command) > 1 and command[1] == "build":
                    return CompletedProcess(command, 0, "", "")
                return CompletedProcess(command, 0, "ok", "")

            errors = run_examples(root, data, "starryc", runner=runner)
            self.assertTrue(any("stdout mismatch" in error for error in errors))

    def test_negative_example_must_match_diagnostic(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = manifest([{"path": "index"}])
            source = "```starry reject\nval x\n```\n\n```text\nprogram.starry:1:7: SemTypeMismatch: type mismatch\n```\n"
            write(root / "docs/en/_index.md", source)
            write(root / "docs/zh-cn/_index.md", source)

            def runner(command, cwd, text_input=None):
                return CompletedProcess(command, 1, "", "")

            errors = run_examples(root, data, "starryc", runner=runner)
            self.assertTrue(any("diagnostic mismatch" in error for error in errors))


if __name__ == "__main__":
    unittest.main()