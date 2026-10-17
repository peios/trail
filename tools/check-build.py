#!/usr/bin/env python3
"""Keep build metadata pins consistent across Cargo, npm, and Bazel."""

import json
import pathlib
import re
import tomllib


def main():
    manifest = tomllib.loads(pathlib.Path("Cargo.toml").read_text())
    module = pathlib.Path("MODULE.bazel").read_text()
    build = pathlib.Path("BUILD.bazel").read_text()
    version = manifest["package"]["version"]
    for text in [module, build]:
        assert re.search(r'version = "' + re.escape(version) + r'"', text)
    toolchain = tomllib.loads(pathlib.Path("rust-toolchain.toml").read_text())
    assert f'versions = ["{toolchain["toolchain"]["channel"]}"]' in module
    package = json.loads(pathlib.Path("package.json").read_text())
    lock = json.loads(pathlib.Path("package-lock.json").read_text())
    prettier = lock["packages"]["node_modules/prettier"]
    assert package["devDependencies"]["prettier"] == prettier["version"]
    assert prettier["resolved"] in module and prettier["integrity"] in module


if __name__ == "__main__":
    main()
