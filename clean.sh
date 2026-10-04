#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'HELP'
Usage: ./clean.sh [--dry-run] [--all] [--help]

Remove repository build artifacts and temporary files.
  --dry-run  List eligible paths without removing anything.
  --all      Also remove local ESP-IDF SDK caches and the documented
             /tmp/weather-wifi-preview and /tmp/weather-assets-venv directories.
  --help     Show this help.

Tracked files, .git, secrets, backups, and symlink targets are preserved.
HELP
}

dry_run=false
clean_all=false
for argument in "$@"; do
    case "$argument" in
        --dry-run) dry_run=true ;;
        --all) clean_all=true ;;
        --help) usage; exit 0 ;;
        *) printf 'Unknown argument: %s\n' "$argument" >&2; usage >&2; exit 2 ;;
    esac
done

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
if ! command -v git >/dev/null 2>&1; then
    printf 'Git is required to protect tracked files.\n' >&2
    exit 1
fi
git_root="$(git -C "$repo_dir" rev-parse --show-toplevel)"
if [[ "$git_root" != "$repo_dir" ]]; then
    printf 'Place clean.sh at the root of the project Git repository.\n' >&2
    exit 1
fi

# Materialize discovery results so Git/find failures stop cleanup before deletion.
inventory_dir="$(mktemp -d)"
trap 'rm -rf -- "$inventory_dir"' EXIT
git -C "$repo_dir" ls-files -z > "$inventory_dir/tracked"
mapfile -d '' -t tracked_files < "$inventory_dir/tracked"

# Do not descend into build/cache directories or protected trees. Matching
# symlinks are listed for reporting, but are never traversed or deleted.
find -P "$repo_dir" \
    \( -name .git -o -name secrets -o -name backups \) -prune -o \
    \( -name target -o -name .embuild -o -name __pycache__ -o -name 'mutants.out*' \) \
        -prune -print0 -o \
    \( -type f -o -type l \) \
        \( -name '*.rs.bk' -o -name '*.pdb' -o -name '*.pyc' -o -name '*.pyo' \
           -o -name sdkconfig -o -name sdkconfig.old -o -name 'sdkconfig.*.old' \) \
        -print0 > "$inventory_dir/candidates"
mapfile -d '' -t candidates < "$inventory_dir/candidates"
if "$clean_all"; then
    candidates+=(/tmp/weather-wifi-preview /tmp/weather-assets-venv)
fi

removed=0
skipped=0
failed=0
for candidate in "${candidates[@]}"; do
    if [[ "${candidate##*/}" == .embuild ]] && ! "$clean_all"; then
        continue
    fi
    if [[ ! -e "$candidate" && ! -L "$candidate" ]]; then
        continue
    fi
    if [[ -L "$candidate" ]]; then
        printf 'Skip symlink: %s\n' "$candidate"
        skipped=$((skipped + 1))
        continue
    fi

    protected=false
    if [[ "$candidate" == "$repo_dir/"* ]]; then
        relative="${candidate#"$repo_dir/"}"
        for tracked in "${tracked_files[@]}"; do
            if [[ "$tracked" == "$relative" || "$tracked" == "$relative/"* ]]; then
                protected=true
                break
            fi
        done
    fi
    if "$protected"; then
        printf 'Skip tracked file or directory containing tracked files: %s\n' "$candidate"
        skipped=$((skipped + 1))
        continue
    fi
    if [[ -d "$candidate" ]]; then
        protected_entry="$(find -P "$candidate" \
            \( -name .git -o -name secrets -o -name backups \) -print -quit)"
        if [[ -n "$protected_entry" ]]; then
            printf 'Skip directory containing a protected tree: %s\n' "$candidate"
            skipped=$((skipped + 1))
            continue
        fi
    fi

    if "$dry_run"; then
        printf 'Would remove: %s\n' "$candidate"
        removed=$((removed + 1))
    elif rm -rf -- "$candidate"; then
        printf 'Removed: %s\n' "$candidate"
        removed=$((removed + 1))
    else
        printf 'Failed to remove: %s\n' "$candidate" >&2
        failed=$((failed + 1))
    fi
done

if "$dry_run"; then
    printf 'Dry run: %d paths eligible, %d skipped.\n' "$removed" "$skipped"
else
    printf 'Cleanup: %d paths removed, %d skipped, %d failed.\n' "$removed" "$skipped" "$failed"
fi
[[ "$failed" -eq 0 ]]
