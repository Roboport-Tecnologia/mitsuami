#!/usr/bin/env bash
# Builds every example of the `mitsuami` crate once per Linux toolkit and
# copies the binaries into one folder per toolkit, so both can be tried side
# by side:
#
#   target/examples/gtk/<example>
#   target/examples/qt/<example>
#
# Usage: scripts/build-examples.sh [--release] [gtk|qt ...]
# With no toolkit named, it builds both. Extra cargo flags go in CARGO_FLAGS.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

profile=debug
profile_flag=()
toolkits=()
for arg in "$@"; do
    case "$arg" in
        --release) profile=release; profile_flag=(--release) ;;
        gtk | qt) toolkits+=("$arg") ;;
        -h | --help) sed -n '2,11s/^# \{0,1\}//p' "$0"; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done
[[ ${#toolkits[@]} -eq 0 ]] && toolkits=(gtk qt)

target_dir="$(cargo metadata --format-version 1 --no-deps \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
out_root="$target_dir/examples"

# Every `examples/<name>.rs` and `examples/<name>/main.rs` is an example.
examples=()
for path in crates/mitsuami/examples/*; do
    if [[ -f "$path" && "$path" == *.rs ]]; then
        examples+=("$(basename "$path" .rs)")
    elif [[ -f "$path/main.rs" ]]; then
        examples+=("$(basename "$path")")
    fi
done

for toolkit in "${toolkits[@]}"; do
    case "$toolkit" in
        gtk) features=(--features gtk) ;;
        qt) features=(--no-default-features --features kde) ;;
    esac

    echo "==> Building ${#examples[@]} examples for $toolkit ($profile)"
    # shellcheck disable=SC2086
    cargo build --package mitsuami --examples "${profile_flag[@]}" "${features[@]}" ${CARGO_FLAGS:-}

    out="$out_root/$toolkit"
    rm -rf "$out"
    mkdir -p "$out"
    for example in "${examples[@]}"; do
        cp "$target_dir/$profile/examples/$example" "$out/"
    done
    echo "    -> $out"
done

echo
echo "Run one with e.g.: $out_root/${toolkits[0]}/${examples[0]}"
