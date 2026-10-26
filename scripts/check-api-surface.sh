#!/usr/bin/env bash
# Guard against "fix" PRs that rewrite whole files from stale bases and
# silently drop public contract functions (issue #985).
#
# Extracts `pub fn` names inside `#[contractimpl]` blocks from the base
# branch and the PR branch. Fails when any public function is removed,
# unless the PR carries the `breaking-change` label.
#
# Usage: scripts/check-api-surface.sh <base-ref> [head-ref]
#   BASE_REF  git ref of the base branch (e.g. origin/main)
#   HEAD_REF  git ref of the PR branch (default: working tree)
#
# Env:
#   PR_LABELS  comma-separated PR labels; `breaking-change` allows removals

set -euo pipefail

BASE_REF="${1:-origin/main}"
HEAD_REF="${2:-}"
CONTRACTS_DIR="contracts"

if ! git rev-parse --verify --quiet "${BASE_REF}" >/dev/null; then
  echo "error: base ref '${BASE_REF}' not found" >&2
  exit 2
fi

# Extract sorted `pub fn` names inside #[contractimpl] blocks from a file.
extract_from_file() {
  awk '
    /#\[contractimpl\]/ { in_impl = 1 }
    in_impl && /pub[[:space:]]+fn[[:space:]]+/ {
      line = $0
      sub(/.*pub[[:space:]]+fn[[:space:]]+/, "", line)
      sub(/[^A-Za-z0-9_].*/, "", line)
      if (line != "") print line
    }
    in_impl && /^}/ { in_impl = 0 }
  ' "$1" | sort -u
}

# Extract the API surface for a given ref (empty ref = working tree).
extract_surface() {
  local ref="$1"
  local file
  while IFS= read -r file; do
    local content
    if [ -z "$ref" ]; then
      content="$(cat "$file")"
    else
      content="$(git show "${ref}:${file}" 2>/dev/null || true)"
    fi
    [ -z "$content" ] && continue
    printf '%s\n' "$content" | awk '
      /#\[contractimpl\]/ { in_impl = 1 }
      in_impl && /pub[[:space:]]+fn[[:space:]]+/ {
        line = $0
        sub(/.*pub[[:space:]]+fn[[:space:]]+/, "", line)
        sub(/[^A-Za-z0-9_].*/, "", line)
        if (line != "") print line
      }
      in_impl && /^}/ { in_impl = 0 }
    ' | sed "s|^|${file}::|"
  done < <(list_contract_files "$ref")
}

# List contract source files for a ref (empty ref = working tree).
list_contract_files() {
  local ref="$1"
  if [ -z "$ref" ]; then
    find "${CONTRACTS_DIR}" -type f -name '*.rs' | sort
  else
    git ls-tree -r --name-only "$ref" -- "${CONTRACTS_DIR}" 2>/dev/null \
      | grep -E '\.rs$' | sort || true
  fi
}

BASE_SURFACE="$(extract_surface "$BASE_REF" | sort -u)"
HEAD_SURFACE="$(extract_surface "$HEAD_REF" | sort -u)"

REMOVED="$(comm -23 <(printf '%s\n' "$BASE_SURFACE") <(printf '%s\n' "$HEAD_SURFACE") | sed '/^$/d')"

if [ -z "$REMOVED" ]; then
  echo "API surface OK: no public contract functions removed."
  exit 0
fi

echo "Public contract functions removed relative to '${BASE_REF}':" >&2
printf '%s\n' "$REMOVED" | sed 's/^/  - /' >&2

case ",${PR_LABELS:-}," in
  *,breaking-change,*)
    echo "'breaking-change' label present; allowing removals." >&2
    exit 0
    ;;
esac

echo "error: public API removed without a 'breaking-change' label." >&2
exit 1
