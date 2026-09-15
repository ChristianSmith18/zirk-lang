#!/usr/bin/env bash
# Verify a browser package against the current clean compiler revision.
set -euo pipefail

readonly root="$(cd "$(dirname "$0")/.." && pwd)"
readonly package="${1:?usage: verify-browser-package.sh PACKAGE_DIRECTORY}"
cd "$root"
if [[ -n "$(git status --porcelain)" ]]; then
    printf 'refusing to verify provenance against a dirty revision\n' >&2
    exit 1
fi
./scripts/check-browser-package-size.sh "$package"
node ./scripts/verify-browser-package.mjs "$package" "$(git rev-parse HEAD)"
