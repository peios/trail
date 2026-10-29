#!/usr/bin/env python3
"""Keep build metadata pins consistent across Cargo, rustup, pekit, and npm."""

import json
import pathlib
import re
import tomllib


def main():
    toolchain = tomllib.loads(pathlib.Path("rust-toolchain.toml").read_text())
    workspace = pathlib.Path("workspace.gcsl").read_text()
    channel = toolchain["toolchain"]["channel"]
    major, minor, _ = channel.split(".")
    # The formatter rustup selects matches the compiler pekit builds with.
    assert re.search(rf"\brust_{major}_{minor}\b", workspace), channel
    package = json.loads(pathlib.Path("package.json").read_text())
    lock = json.loads(pathlib.Path("package-lock.json").read_text())
    prettier = lock["packages"]["node_modules/prettier"]
    assert package["devDependencies"]["prettier"] == prettier["version"]
    pins = pathlib.Path("npm.pins").read_text()
    assert prettier["resolved"] in pins, "run `pekit run pin_packages`"


if __name__ == "__main__":
    main()
