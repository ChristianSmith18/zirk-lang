#!/usr/bin/env bash
#
# Canonical-format gate: tracked .zrk sources and parseable ```zirk fences in
# docs/ must already be in canonical formatter output.
#
# Excluded on purpose:
#   - examples/structured_tasks_examples.zrk documents a roadmap slice that
#     does not compile yet.
#   - docs/concurrency-model-draft.zrk and the invalid browser fixture do not
#     parse by design and are outside the gate's target set.
#
# Usage: ./scripts/check-format.sh [zirk-binary]

set -euo pipefail

cd "$(dirname "$0")/.."

bin="${1:-target/debug/zirk}"

targets=()
while IFS= read -r file; do
    targets+=("$file")
done < <(
    find crates/zirk-cli/tests/corpus/valid examples -name '*.zrk' \
        ! -name 'structured_tasks_examples.zrk' | sort
)

"$bin" format --check "${targets[@]}"

node scripts/format-doc-examples.mjs --check "$bin"
