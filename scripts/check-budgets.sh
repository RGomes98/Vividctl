#!/usr/bin/env bash
# Checks the build budgets of docs/adr/0006-cli-parser-and-budgets.md against the `vividctl` binary.
#
#   scripts/check-budgets.sh
#
# Builds `vivid-cli` in release mode and fails when the stripped binary is bigger than
# MAX_CLI_BYTES or the dependency tree has more than MAX_CLI_CRATES unique crates (workspace
# crates included). Both can be overridden from the environment, which is how the check itself is
# tested. Build time is recorded in the ADR but not enforced: it is too noisy on shared runners.
set -euo pipefail

MAX_CLI_BYTES="${MAX_CLI_BYTES:-1572864}"   # 1.5 MiB
MAX_CLI_CRATES="${MAX_CLI_CRATES:-40}"

cd "$(dirname "$0")/.."
cargo build --release --package vivid-cli --quiet

size="$(stat -c %s target/release/vividctl)"
crates="$(cargo tree --package vivid-cli --edges normal --prefix none | sort -u | wc -l)"

printf 'vividctl binary: %s bytes (budget %s)\n' "$size" "$MAX_CLI_BYTES"
printf 'crates in the dependency tree: %s (budget %s)\n' "$crates" "$MAX_CLI_CRATES"

status=0
if [ "$size" -gt "$MAX_CLI_BYTES" ]; then
    echo "FAIL: the vividctl binary is over its size budget" >&2
    status=1
fi
if [ "$crates" -gt "$MAX_CLI_CRATES" ]; then
    echo "FAIL: vividctl depends on too many crates" >&2
    status=1
fi
[ "$status" -eq 0 ] && echo "within budget"
exit "$status"
