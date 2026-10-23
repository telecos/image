#!/usr/bin/env python3
"""Prepare approved external ITU T.88 vectors without redistributing them."""

import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import zipfile


ROOT = Path(__file__).resolve().parents[2]
PACKAGE_SHA256 = "e2a3c458c4d28d5a835c0815fd9b23e197dbc1b83fe81d8e8d8b2ac413d8f22f"
CONFORMANCE_SHA256 = "fe3de9056d94b20db3a6ab17032c621a7f05b6ecb1db84b4b58d0825fd6b7033"
MANIFEST_SHA256 = "e9ee986db84fdfe960d56dedb843d3c16ea3e12d2a2f97f6cf56e96f55372ff1"
MANIFEST = ROOT.parent / "jbig2_rs/validation/conformance/itu-t88-2018-manifest.json"


def prepare(archive: Path) -> Path:
    data = archive.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest == PACKAGE_SHA256:
        with zipfile.ZipFile(io.BytesIO(data)) as package:
            data = package.read("Software/JBIG2_ConformanceData-A20180829.zip")
    if hashlib.sha256(data).hexdigest() != CONFORMANCE_SHA256:
        raise ValueError("official ITU conformance archive SHA-256 mismatch")

    manifest_data = MANIFEST.read_bytes()
    if hashlib.sha256(manifest_data).hexdigest() != MANIFEST_SHA256:
        raise ValueError("sibling jbig2_rs ITU manifest SHA-256 mismatch")
    manifest = json.loads(manifest_data)
    target = (ROOT / "target").resolve()
    output = target / "jbig2-conformance/files"
    prepared = []
    with zipfile.ZipFile(io.BytesIO(data)) as source:
        for entry in manifest["files"]:
            path = PurePosixPath(entry["path"])
            if path.is_absolute() or ".." in path.parts or len(path.parts) != 2:
                raise ValueError(f"invalid manifest path: {path}")
            destination = (output / path).resolve()
            if not destination.is_relative_to(target):
                raise ValueError(f"output escapes ignored target directory: {path}")
            contents = source.read(path.as_posix())
            if (
                len(contents) != entry["size_bytes"]
                or hashlib.sha256(contents).hexdigest() != entry["sha256"]
            ):
                raise ValueError(f"official vector hash/size mismatch: {path}")
            prepared.append((destination, contents))
    for destination, contents in prepared:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(contents)
    return output / "JBIG2_ConformanceData-A20180829"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    arguments = parser.parse_args()
    try:
        root = prepare(arguments.archive)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.error(str(error))
    print(f"Prepared approved external vectors: {root}")
    print(f"JBIG2_ITU_VECTOR_ROOT={root}")


if __name__ == "__main__":
    main()
