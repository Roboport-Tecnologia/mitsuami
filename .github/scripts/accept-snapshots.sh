#!/usr/bin/env bash
# Accepts the snapshots and visual baselines a CI run wrote for review:
# downloads every OS's `snapshots-*` artifact and moves each `<file>.new` and
# `<name>.new.png` in it over its baseline. Look at them before committing.
# Usage: accept-snapshots.sh <run id>
set -euo pipefail

run=${1:?usage: accept-snapshots.sh <run id>}
root=$(git rev-parse --show-toplevel)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pattern='^crates/[^/]+/tests/(snapshots|visual)/.*\.(new|new\.png|diff\.png)$'
gh run download "$run" --pattern 'snapshots-*' --dir "$tmp"
for archive in "$tmp"/*/snapshots.tar; do
  [ -e "$archive" ] || { echo "run $run has no snapshots to review"; exit 1; }
  # Only snapshots and baselines come out: anything else in the archive
  # (a build script, a workflow) would otherwise land in the checkout.
  tar -tf "$archive" | grep -E "$pattern" | grep -v '/\.\./' > "$tmp/files" || true
  [ -s "$tmp/files" ] || continue
  # Into a folder of its own first, then only its regular files: find
  # doesn't follow links, so a link in the archive can't become a baseline
  # that points out of the checkout, for a later test run to write through.
  stage=$(mktemp -d "$tmp/stage.XXXXXX")
  tar -xf "$archive" -C "$stage" -T "$tmp/files"
  (cd "$stage" && find crates -type f) | grep -E "$pattern" > "$tmp/found" || true
  while IFS= read -r file; do
    case "$file" in
      *.diff.png) rm -f "$root/$file"; continue ;;
      *.new.png) baseline=${file%.new.png}.png ;;
      *.new) baseline=${file%.new} ;;
    esac
    mkdir -p "$(dirname "$root/$baseline")"
    mv -f "$stage/$file" "$root/$baseline" && echo "accepted $baseline"
  done < "$tmp/found"
done
