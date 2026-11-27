"""Generate a deterministic llms.txt index for public Starry documentation."""

from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path

LOCALE_LABELS = {"en": "English", "zh-cn": "Simplified Chinese"}


def parse_frontmatter(path: Path) -> dict:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("+++"):
        raise ValueError(f"{path} does not start with TOML frontmatter")
    end = text.find("+++", 3)
    if end < 0:
        raise ValueError(f"{path} has unterminated TOML frontmatter")
    return tomllib.loads(text[3:end].strip())


def generate(root: Path, output: Path) -> None:
    with (root / "site/data/manifest.json").open("r", encoding="utf-8") as file:
        manifest = json.load(file)
    if manifest.get("versions") != []:
        raise ValueError("llms.txt cannot be generated with versions")
    lines = ["# Starry docs", "", "Current Starry language documentation. No version history.", ""]
    for page in manifest["pages"]:
        lines.append(f"## {page['path']}")
        lines.append("")
        for locale in manifest["locales"]:
            file = root / "docs" / locale / ("_index.md" if page["path"] == "index" else f"{page['path']}.md")
            metadata = parse_frontmatter(file)
            title = metadata["title"]
            description = metadata["description"]
            url = f"/{locale}/" if page["path"] == "index" else f"/{locale}/{page['path']}/"
            lines.append(f"- [{locale} / {title}]({url}) — {description}")
        lines.append("")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    generate(args.root, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())