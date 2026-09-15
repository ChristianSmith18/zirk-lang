#!/usr/bin/env bash
# Enforce reviewed browser artifact budgets. Values are bytes.
set -euo pipefail

readonly package="${1:?usage: check-browser-package-size.sh PACKAGE_DIRECTORY}"
readonly wasm="$package/zirk_browser_bg.wasm"
readonly warning_raw=3000000
readonly error_raw=4000000
readonly warning_gzip=1000000
readonly error_gzip=1300000
readonly warning_brotli=900000
readonly error_brotli=1200000

[[ -f "$wasm" ]] || { printf 'missing WASM artifact: %s\n' "$wasm" >&2; exit 1; }

measure() {
    local name="$1" value="$2" warning="$3" error="$4"
    if (( value > error )); then
        printf '%s size %s exceeds error budget %s\n' "$name" "$value" "$error" >&2
        exit 1
    fi
    if (( value > warning )); then
        printf 'warning: %s size %s exceeds reviewed warning budget %s\n' "$name" "$value" "$warning" >&2
    fi
    printf '%s=%s\n' "$name" "$value"
}

raw="$(stat -f%z "$wasm")"
gzip_size="$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')"
brotli_size="$(brotli -q 11 -c "$wasm" | wc -c | tr -d ' ')"
measure raw "$raw" "$warning_raw" "$error_raw"
measure gzip "$gzip_size" "$warning_gzip" "$error_gzip"
measure brotli "$brotli_size" "$warning_brotli" "$error_brotli"
