#!/usr/bin/env bash
# Runs the same invocations against every CLI candidate and compares what they print on success and
# the exit code on failure. Help and error texts are allowed to differ in wording.
# Usage: ./check.sh <dir-with-candidate-binaries>   (binaries named after their folder)
set -uo pipefail
bins="${1:?usage: check.sh <dir with binaries named clap-derive, clap-builder, lexopt, pico-args, std-only>}"
names=(clap-derive clap-builder lexopt pico-args std-only)

# invocation | expected result: ok (prints the parsed command) or fail (exit status 2)
cases=(
  "list|ok" "list --json|ok" "get|ok" "get --output PHL:PHL 242V8:0x00008fac|ok"
  "set 512|ok" "set -300|ok" "set 512 --output DEL:U2720Q:0x1#2|ok" "set --output X 1023|ok"
  "reset|ok" "reset --output X|ok" "doctor|ok"
  "set|fail" "set abc|fail" "set 99999|fail" "bogus|fail" "list --output X|fail" "doctor --json|fail"
  "get --output|fail" "get extra|fail" "list --json --json|ok-or-fail"
)

status=0
for entry in "${cases[@]}"; do
    args="${entry%|*}"; want="${entry##*|}"
    # words with spaces in the case text are kept together by quoting the identity after --output
    read -r -a argv <<<"$(echo "$args" | sed -E 's/PHL 242V8/PHL_242V8/')"
    outs=(); codes=()
    for n in "${names[@]}"; do
        out="$("$bins/$n" "${argv[@]}" 2>/dev/null)"; code=$?
        outs+=("$out"); codes+=("$code")
    done
    if [ "$want" = ok ]; then
        for i in 1 2 3 4; do
            [ "${outs[$i]}" = "${outs[0]}" ] && [ "${codes[$i]}" = 0 ] || { echo "MISMATCH ok-case '$args': ${names[$i]} -> ${codes[$i]} '${outs[$i]}' vs '${outs[0]}'"; status=1; }
        done
        [ "${codes[0]}" = 0 ] || { echo "MISMATCH ok-case '$args': ${names[0]} exited ${codes[0]}"; status=1; }
    elif [ "$want" = fail ]; then
        for i in 0 1 2 3 4; do
            [ "${codes[$i]}" = 2 ] || { echo "MISMATCH fail-case '$args': ${names[$i]} exited ${codes[$i]} (want 2)"; status=1; }
        done
    fi
done
[ "$status" = 0 ] && echo "all candidates agree on ${#cases[@]} invocations"
exit "$status"
