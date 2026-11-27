"""Checks the Starry public documentation as a finite, bilingual corpus."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

MANIFEST_PATH = Path("site/data/manifest.json")
ROOT = Path(__file__).resolve().parents[2]
PAGE_TYPES = {"overview", "tutorial", "reference", "spec", "glossary"}
ROLES = {"newcomer", "lookup", "builder", "debugger", "all"}
FORBIDDEN_TERMS = (
    "docs/syntax/",
    "crates/",
    "runtime/",
    ".rs",
    "feature-matrix",
    "parser-notes",
    "semantic-notes",
    "coverage.md",
    "opt-notes",
    "roadmap",
    "release notes",
    "migration",
    "legacy",
    "archive",
    "TODO",
    "TBD",
)
LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
FENCE_RE = re.compile(r"^```([^\n]*)\n(.*?)\n?```$", re.DOTALL | re.MULTILINE)
CODE_FENCE_LANGUAGES = {"starry", "bash", "toml", "text", "ebnf"}
EXECUTABLE_STARRY_KINDS = {"check", "run", "reject"}
NON_EXECUTABLE_STARRY_KINDS = {"syntax"}
def _real_runner(command: list[str], cwd: str, text_input: str | None = None):
    return subprocess.run(command, cwd=cwd, text=True, input=text_input, capture_output=True)


@dataclass(frozen=True)
class Fence:
    info: str
    code: str


def load_manifest(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as file:
        return json.load(file)


def page_path(root: Path, locale: str, relative_path: str) -> Path:
    filename = "_index.md" if relative_path == "index" else f"{relative_path}.md"
    return root / "docs" / locale / filename


def extract_fences(text: str) -> list[Fence]:
    return [Fence(info.strip(), code) for info, code in FENCE_RE.findall(text)]


def validate_example_pairs(page: str, fences: list[Fence]) -> list[str]:
    errors: list[str] = []
    for index, fence in enumerate(fences):
        if not fence.info.startswith("starry") or fence.info[len("starry"):].strip() in NON_EXECUTABLE_STARRY_KINDS:
            continue
        mode = fence.info[len("starry"):].strip()
        if mode not in EXECUTABLE_STARRY_KINDS and mode not in NON_EXECUTABLE_STARRY_KINDS:
            errors.append(f"{page}: unsupported Starry fence {fence.info!r}")
        if mode in {"run", "reject"}:
            if index + 1 >= len(fences) or fences[index + 1].info != "text":
                errors.append(f"{page}: {fence.info!r} must be followed by a text fence")
    return errors


def validate_manifest(manifest: dict, root: Path) -> list[str]:
    errors: list[str] = []
    if manifest.get("schema") != 1:
        errors.append("manifest.schema must be 1")
    locales = manifest.get("locales")
    if not isinstance(locales, list) or not locales or len(set(locales)) != len(locales):
        errors.append("manifest.locales must be a non-empty unique list")
    if manifest.get("defaultLocale") not in (locales or []):
        errors.append("manifest.defaultLocale must be one of locales")
    if manifest.get("versions") != []:
        errors.append("manifest.versions must remain an empty list")
    pages = manifest.get("pages")
    if not isinstance(pages, list):
        pages = []
        errors.append("manifest.pages must be a list")
    ids: set[str] = set()
    paths: set[str] = set()
    for index, page in enumerate(pages):
        page_id = page.get("id")
        page_path_value = page.get("path")
        if not isinstance(page_id, str) or not page_id:
            errors.append(f"pages[{index}].id must be non-empty")
        elif page_id in ids:
            errors.append(f"pages[{index}] id is duplicated: {page_id}")
        else:
            ids.add(page_id)
        if (
            not isinstance(page_path_value, str)
            or not page_path_value
            or page_path_value.startswith("/")
            or ".." in Path(page_path_value).parts
            or page_path_value.endswith(".md")
        ):
            errors.append(f"pages[{index}].path is invalid: {page_path_value!r}")
        elif page_path_value in paths:
            errors.append(f"pages[{index}] path is duplicated: {page_path_value}")
        else:
            paths.add(page_path_value)
        if page.get("type") not in PAGE_TYPES:
            errors.append(f"pages[{index}].type is invalid: {page.get('type')!r}")
        roles = page.get("roles")
        if not isinstance(roles, list) or not roles or any(role not in ROLES for role in roles):
            errors.append(f"pages[{index}].roles is invalid")
    if isinstance(locales, list):
        for locale in locales:
            locale_dir = root / "docs" / locale
            if not locale_dir.is_dir():
                continue
            allowed = {"_index.md" if page.get("path") == "index" else f"{page.get('path')}.md" for page in pages if isinstance(page.get("path"), str)}
            actual = {str(path.relative_to(locale_dir)).replace("\\", "/") for path in locale_dir.rglob("*.md")}
            for extra in sorted(actual - allowed):
                errors.append(f"{locale}: unlisted page {extra}")
    return errors


def check_parity(root: Path, manifest: dict) -> list[str]:
    errors: list[str] = []
    for page in manifest.get("pages", []):
        relative_path = page.get("path", "")
        locale_fences: dict[str, list[Fence]] = {}
        for locale in manifest.get("locales", []):
            file = page_path(root, locale, relative_path)
            if not file.is_file():
                errors.append(f"{locale}/{page_path(root, locale, relative_path).name} is missing")
                continue
            locale_fences[locale] = extract_fences(file.read_text(encoding="utf-8"))
        if len(locale_fences) < 2:
            continue
        first_locale = manifest.get("locales", [])[0]
        first = locale_fences.get(first_locale, [])
        for locale in manifest.get("locales", [])[1:]:
            other = locale_fences.get(locale, [])
            if len(first) != len(other):
                errors.append(f"{locale}/{relative_path}.md has {len(other)} code fences; {first_locale} has {len(first)}")
                continue
            for index, (expected, actual) in enumerate(zip(first, other)):
                if (expected.info, expected.code) != (actual.info, actual.code):
                    errors.append(f"{locale}/{relative_path}.md code fence {index + 1} differs from {first_locale}")
    return errors


def _public_markdown(root: Path, manifest: dict) -> list[Path]:
    files: list[Path] = []
    for locale in manifest.get("locales", []):
        locale_dir = root / "docs" / locale
        if locale_dir.is_dir():
            files.extend(locale_dir.rglob("*.md"))
    return sorted(files)


def check_terms(root: Path, manifest: dict) -> list[str]:
    errors: list[str] = []
    for file in _public_markdown(root, manifest):
        text = file.read_text(encoding="utf-8")
        lowered = text.lower()
        for term in FORBIDDEN_TERMS:
            if term.lower() in lowered:
                errors.append(f"{file.relative_to(root)} contains forbidden term {term!r}")
    return errors


def check_links(root: Path, manifest: dict) -> list[str]:
    errors: list[str] = []
    for file in _public_markdown(root, manifest):
        for match in LINK_RE.finditer(file.read_text(encoding="utf-8")):
            target = match.group(1).strip()
            if target.startswith(("http://", "https://")):
                errors.append(f"{file.relative_to(root)} links to external URL {target!r}")
                continue
            if target.startswith("#") or not target:
                continue
            clean = target.split("#", 1)[0]
            if clean.startswith("/"):
                candidate = root / "docs" / manifest["defaultLocale"] / clean.lstrip("/")
            else:
                candidate = file.parent / clean
            candidate = candidate.resolve()
            docs_root = (root / "docs" / file.relative_to(root / "docs").parts[0]).resolve()
            try:
                candidate.relative_to(docs_root)
            except ValueError:
                errors.append(f"{file.relative_to(root)} escapes the public docs: {target!r}")
                continue
            if candidate.is_file():
                continue
            if not candidate.suffix and (candidate.with_suffix(".md").is_file() or (candidate / "index.md").is_file()):
                continue
            errors.append(f"{file.relative_to(root)} has missing link {target!r}")
    return errors


def _diagnostic(text: str) -> str:
    normalized = _normalized(text)
    marker = "program.starry"
    return normalized[normalized.find(marker):] if marker in normalized else normalized

def _normalized(text: str) -> str:
    return text.replace("\r\n", "\n").replace("\r", "\n").strip()


def _run_one_example(
    page: str,
    fence: Fence,
    expected: Fence | None,
    starryc: str,
    runner,
) -> list[str]:
    errors: list[str] = []
    mode = fence.info[len("starry"):].strip()
    with tempfile.TemporaryDirectory(prefix="starry-doc-example-") as directory:
        cwd = Path(directory)
        source = cwd / "program.starry"
        source.write_text(fence.code + "\n", encoding="utf-8")
        command = [starryc, "check", str(source)]
        completed = runner(command, cwd=str(cwd), text_input=None)
        if mode == "reject":
            output = _diagnostic(completed.stderr or completed.stdout)
            if completed.returncode == 0:
                errors.append(f"{page}: reject example unexpectedly compiled")
            elif expected is None or output != _normalized(expected.code):
                errors.append(f"{page}: diagnostic mismatch\nexpected:\n{_normalized(expected.code if expected else '')}\nactual:\n{output}")
            return errors
        if completed.returncode != 0:
            output = _diagnostic(completed.stderr or completed.stdout)
            errors.append(f"{page}: check failed\n{output}")
            return errors
        if mode == "check":
            return errors
        output_name = "program"
        if Path(starryc).suffix.lower() == ".exe":
            output_name += ".exe"
        completed = runner([starryc, "build", str(source), str(cwd / output_name)], cwd=str(cwd), text_input=None)
        if completed.returncode != 0:
            errors.append(f"{page}: build failed\n{_normalized(completed.stderr or completed.stdout)}")
            return errors
        completed = runner([str(cwd / output_name)], cwd=str(cwd), text_input=None)
        if completed.returncode != 0:
            errors.append(f"{page}: program exited {completed.returncode}\n{_normalized(completed.stderr)}")
            return errors
        if expected is None or _normalized(completed.stdout) != _normalized(expected.code):
            errors.append(f"{page}: stdout mismatch\nexpected:\n{_normalized(expected.code if expected else '')}\nactual:\n{_normalized(completed.stdout)}")
    return errors


def run_examples(root: Path, manifest: dict, starryc: str, runner=_real_runner) -> list[str]:
    errors: list[str] = []
    for page in manifest.get("pages", []):
        relative_path = page.get("path", "")
        file = page_path(root, manifest.get("defaultLocale", "en"), relative_path)
        if not file.is_file():
            continue
        page_name = f"{manifest.get('defaultLocale', 'en')}/{relative_path}.md"
        fences = extract_fences(file.read_text(encoding="utf-8"))
        errors.extend(validate_example_pairs(page_name, fences))
        for index, fence in enumerate(fences):
            if not fence.info.startswith("starry") or fence.info[len("starry"):].strip() in NON_EXECUTABLE_STARRY_KINDS:
                continue
            mode = fence.info[len("starry"):].strip()
            expected = fences[index + 1] if mode in {"run", "reject"} and index + 1 < len(fences) else None
            errors.extend(_run_one_example(page_name, fence, expected, starryc, runner))
    return errors


SPEC_INCLUDES = {
    "spec/grammar": (
        '{{< readfile path="syntax/starry.ebnf" lang="ebnf" >}}',
        Path("docs/syntax/starry.ebnf"),
    ),
    "spec/lexicon": (
        '{{< include "syntax/generated/lexicon.md" >}}',
        Path("docs/syntax/generated/lexicon.md"),
    ),
}


def check_spec(root: Path, manifest: dict, syntax_check: bool = True) -> list[str]:
    errors: list[str] = []
    for page_path_value, (required_include, canonical_path) in SPEC_INCLUDES.items():
        canonical = root / canonical_path
        if not canonical.is_file():
            errors.append(f"spec canonical source is missing: {canonical_path}")
        snippets: dict[str, bool] = {}
        for locale in manifest.get("locales", []):
            file = page_path(root, locale, page_path_value)
            if not file.is_file():
                errors.append(f"{locale}/{page_path_value}.md is missing")
                continue
            text = file.read_text(encoding="utf-8")
            snippets[locale] = required_include in text
            if not snippets[locale]:
                errors.append(f"{locale}/{page_path_value}.md missing required include {required_include!r}")
        if len(set(snippets.values())) > 1:
            errors.append(f"{page_path_value} spec includes differ across locales")
    if syntax_check:
        completed = subprocess.run(
            ["cargo", "run", "-p", "syntaxgen", "--", "--check"],
            cwd=root,
            text=True,
            capture_output=True,
        )
        if completed.returncode != 0:
            output = completed.stdout + completed.stderr
            errors.append(f"syntaxgen check failed: {_normalized(output)}")
    return errors


def check_llms(root: Path, manifest: dict, output: Path | None = None) -> list[str]:
    file = output or root / "site/public/llms.txt"
    if not file.is_file():
        return [f"llms.txt is missing: {file.relative_to(root)}"]
    text = file.read_text(encoding="utf-8")
    errors: list[str] = []
    lowered = text.lower()
    for term in FORBIDDEN_TERMS:
        if term.lower() in lowered:
            errors.append(f"llms.txt contains forbidden term {term!r}")
    for page in manifest.get("pages", []):
        path = page.get("path", "")
        for locale in manifest.get("locales", []):
            entry = f"](/{locale}/)" if path == "index" else f"](/{locale}/{path}/)"
            count = text.count(entry)
            if count != 1:
                errors.append(f"llms.txt has {count} entries for {locale}/{path}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=("manifest", "parity", "terms", "links", "examples", "spec", "llms", "all"),
    )
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--starryc", type=Path, default=ROOT / "target/debug/starryc")
    args = parser.parse_args()
    manifest = load_manifest(args.root / MANIFEST_PATH)
    checks = {
        "manifest": lambda: validate_manifest(manifest, args.root),
        "parity": lambda: check_parity(args.root, manifest),
        "terms": lambda: check_terms(args.root, manifest),
        "links": lambda: check_links(args.root, manifest),
        "examples": lambda: run_examples(args.root, manifest, str(args.starryc)),
        "spec": lambda: check_spec(args.root, manifest),
        "llms": lambda: check_llms(args.root, manifest),
    }
    names = list(checks) if args.command == "all" else [args.command]
    if args.command == "all":
        names.remove("llms")
    errors: list[str] = []
    for name in names:
        found = checks[name]()
        errors.extend(f"{name}: {error}" for error in found)
    if errors:
        for error in errors:
            print(error)
        return 1
    print("docs check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())