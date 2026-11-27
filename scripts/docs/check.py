"""Checks the Starry public documentation as a finite, bilingual corpus."""

from __future__ import annotations

import argparse
import json
import re
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


def load_manifest(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as file:
        return json.load(file)


def page_path(root: Path, locale: str, relative_path: str) -> Path:
    return root / "docs" / locale / f"{relative_path}.md"


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
        locale_paths = [root / "docs" / locale for locale in locales]
        for locale, locale_dir in zip(locales, locale_paths):
            if not locale_dir.is_dir():
                continue
            allowed = {f"{page.get('path')}.md" for page in pages if isinstance(page.get("path"), str)}
            actual = {str(path.relative_to(locale_dir)).replace("\\", "/") for path in locale_dir.rglob("*.md")}
            for extra in sorted(actual - allowed):
                errors.append(f"{locale}: unlisted page {extra}")
    return errors


def check_parity(root: Path, manifest: dict) -> list[str]:
    errors: list[str] = []
    for page in manifest.get("pages", []):
        relative_path = page.get("path", "")
        for locale in manifest.get("locales", []):
            if not page_path(root, locale, relative_path).is_file():
                errors.append(f"{locale}/{relative_path}.md is missing")
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
            docs_root = (root / "docs" / manifest["defaultLocale"]).resolve()
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("manifest", "parity", "terms", "links", "all"))
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    manifest = load_manifest(args.root / MANIFEST_PATH)
    checks = {
        "manifest": validate_manifest,
        "parity": check_parity,
        "terms": check_terms,
        "links": check_links,
    }
    names = list(checks) if args.command == "all" else [args.command]
    errors: list[str] = []
    for name in names:
        found = (
        validate_manifest(manifest, args.root)
        if name == "manifest"
        else checks[name](args.root, manifest)
    )
        errors.extend(f"{name}: {error}" for error in found)
    if errors:
        for error in errors:
            print(error)
        return 1
    print("docs check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())