#!/usr/bin/env bash
# Guard against contracts whose instance entry is never bumped (issue #1106).
#
# The contract instance entry holds the admin, id counters, registry pointers,
# config *and* the contract-instance record itself. If it is archived, every
# call to the contract fails until someone submits a restore. Every crate that
# writes instance storage (directly or through `shared::counters::safe_increment*`)
# — and every deployable contract crate, since each one owns an instance entry —
# must therefore call `ttl_config::extend_instance_ttl` (or bump the instance
# directly with `instance().extend_ttl`) from its entry points.
#
# Usage: scripts/check-instance-ttl.sh

set -euo pipefail

CONTRACTS_DIR="${CONTRACTS_DIR:-contracts}"
status=0
checked=0

for manifest in "${CONTRACTS_DIR}"/Cargo.toml "${CONTRACTS_DIR}"/*/Cargo.toml; do
  crate_dir=$(dirname "$manifest")
  src_dir="${crate_dir}/src"
  [ -d "$src_dir" ] || continue
  # The ttl-config crate defines the helper; skip it.
  [ "$(basename "$crate_dir")" = "ttl-config" ] && continue

  # Every deployable contract owns an instance entry (and most write admin,
  # counters or registry pointers into it). Library crates without entry
  # points (e.g. `shared`, whose `counters::safe_increment*` write instance
  # storage on behalf of the caller) cannot bump on their own and are skipped.
  if ! grep -qE 'cdylib' "$manifest" || ! grep -rq '#\[contractimpl' "$src_dir"; then
    continue
  fi

  checked=$((checked + 1))
  # `instance().extend_ttl(` is accepted for crates that bump with their own
  # constants (e.g. liquidity-pool).
  if ! grep -rqE 'extend_instance_ttl|instance\(\)\s*\.extend_ttl\(' "$src_dir"; then
    echo "::error file=${manifest}::contract crate '$(basename "$crate_dir")' never calls ttl_config::extend_instance_ttl"
    status=1
  fi
done

echo "Checked ${checked} contract crate(s) for extend_instance_ttl."
exit $status
