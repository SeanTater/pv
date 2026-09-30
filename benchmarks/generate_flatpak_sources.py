#!/usr/bin/env python3
"""Regenerate Flatpak's offline crate sources from the checked-in Cargo.lock."""

import json
import tomllib
from pathlib import Path

root = Path(__file__).resolve().parent.parent
lock = tomllib.loads((root / "Cargo.lock").read_text())
sources = []
for package in lock["package"]:
    if not package.get("source", "").startswith("registry+"):
        continue
    name, version, checksum = package["name"], package["version"], package["checksum"]
    destination = f"cargo/vendor/{name}-{version}"
    sources.extend(
        [
            {
                "type": "archive",
                "archive-type": "tar-gzip",
                "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                "sha256": checksum,
                "dest": destination,
            },
            {
                "type": "inline",
                "contents": json.dumps({"package": checksum, "files": {}}),
                "dest": destination,
                "dest-filename": ".cargo-checksum.json",
            },
        ]
    )
sources.append(
    {
        "type": "inline",
        "dest": "cargo",
        "dest-filename": "config.toml",
        "contents": '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n[source.crates-io]\nreplace-with = "vendored-sources"\n',
    }
)
(root / "generated-sources.json").write_text(json.dumps(sources, indent=4) + "\n")
