#!/usr/bin/env bash

set -euo pipefail

zirk_repository_root="$(cd "$(dirname "$0")/.." && pwd)"
zirk_site_root="${ZIRK_LANG_SITE_PATH:-"$zirk_repository_root/../zirk-lang-site"}"

if [[ ! -f "$zirk_site_root/package.json" || ! -f "$zirk_site_root/content/handbook-source.json" ]]; then
    printf 'zirk-lang-site was not found at %s\n' "$zirk_site_root" >&2
    printf 'Set ZIRK_LANG_SITE_PATH to the companion repository checkout.\n' >&2
    exit 1
fi

if [[ "${ZIRK_LANG_SITE_ALLOW_DIRTY:-0}" != "1" ]]; then
    site_changes="$(git -C "$zirk_site_root" status --porcelain)"
    if [[ -n "$site_changes" ]]; then
        printf 'zirk-lang-site has uncommitted changes; refusing to mix synchronization output:\n%s\n' "$site_changes" >&2
        printf 'Commit/stash them, or deliberately set ZIRK_LANG_SITE_ALLOW_DIRTY=1.\n' >&2
        exit 1
    fi
fi

printf 'Synchronizing public documentation into %s\n' "$zirk_site_root"
yarn --cwd "$zirk_site_root" content:sync --source "$zirk_repository_root" "$@"

printf '\nWebsite synchronization completed. Review and validate the companion diff:\n'
git -C "$zirk_site_root" status --short
