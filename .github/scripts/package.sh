#!/usr/bin/env bash
# Package a release build into dist/tuipr-<version>-<target>.tar.gz plus a
# .sha256 file that `shasum -a 256 -c` accepts.
#
# Usage: package.sh <target-triple> <version>
# Expects the binary at target/<target-triple>/release/tuipr.
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 <target-triple> <version>" >&2
    exit 2
fi
target=$1
version=$2
binary="target/${target}/release/tuipr"
name="tuipr-${version}-${target}"

if [[ ! -x "$binary" ]]; then
    echo "missing binary: $binary" >&2
    exit 1
fi

checksum() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$@"
    else
        shasum -a 256 "$@"
    fi
}

rm -rf "dist/${name}" "dist/${name}.tar.gz" "dist/${name}.tar.gz.sha256"
mkdir -p "dist/${name}"
cp "$binary" LICENSE README.md "dist/${name}/"
tar -C dist -czf "dist/${name}.tar.gz" "$name"
rm -rf "dist/${name}"
(cd dist && checksum "${name}.tar.gz" > "${name}.tar.gz.sha256")
echo "dist/${name}.tar.gz"
