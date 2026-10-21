#!/usr/bin/env bash
# Runs the install part of the README quick start, as written, on a clean Ubuntu in a container, as
# a normal user with sudo and without ~/.local/bin, then checks that `vividctl` is found in a new
# terminal: a bash login shell, an interactive bash, and a zsh login shell.
#
#   scripts/check-quick-start.sh [ubuntu-tag]          24.04 (default) or 26.04
#
# The block is the code between <!-- quick-start --> and <!-- /quick-start --> in README.md, up to
# its first blank line (the lines after it need a monitor). Two things differ from a real machine,
# and only these: `apt` answers yes by itself, and `git clone` gets the commit under test instead of
# GitHub's main (so uncommitted changes are not part of the run). QUICK_START_BLOCK=file runs
# another block instead, to try a change before editing the README.
set -euo pipefail

cd "$(dirname "$0")/.."
tag="${1:-24.04}"
url="https://github.com/RGomes98/Vividctl.git"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [ -n "${QUICK_START_BLOCK:-}" ]; then
    cp "$QUICK_START_BLOCK" "$work/install.sh"
else
    sed -n '/^<!-- quick-start -->$/,/^<!-- \/quick-start -->$/p' README.md \
        | sed -n '/^```/,/^```/p' | sed '1d;$d' | sed '/^$/,$d' > "$work/install.sh"
fi
if [ ! -s "$work/install.sh" ]; then
    echo "error: no quick start block between <!-- quick-start --> markers in README.md" >&2
    exit 1
fi
if ! grep -qF "$url" "$work/install.sh"; then
    echo "error: the quick start no longer clones $url; update this script" >&2
    exit 1
fi
# Same last path component as the real URL, so `git clone` makes the same directory.
sed -i "s#$url#/srv/Vividctl.git#" "$work/install.sh"
echo "--- the block under test:"
cat "$work/install.sh"

# The commit under test, as a repository whose main branch is that commit (works from a detached HEAD).
git init -q --bare "$work/repo.git"
git -C "$work/repo.git" symbolic-ref HEAD refs/heads/main
# CI checks out a shallow clone; a fresh repository refuses a push from one unless told otherwise.
git -C "$work/repo.git" config receive.shallowUpdate true
git push -q "$work/repo.git" "HEAD:refs/heads/main"

cat > "$work/check.sh" <<'EOF'
#!/bin/bash
fails=0
check() {
    local label="$1"; shift
    local out
    if out="$("$@" 2>&1)"; then echo "PASS  $label: $(tail -1 <<<"$out")"; else echo "FAIL  $label: $(tail -1 <<<"$out")"; fails=$((fails + 1)); fi
}
# git refuses to read a repository another user owns, so hand the test user its own copy.
cp -r /repo.git /srv/Vividctl.git && chown -R tester: /srv/Vividctl.git
echo "--- running it as a new user (no ~/.local/bin: $([ -e /home/tester/.local/bin ] && echo present || echo absent))"
if su - tester -c 'cd ~ && bash -e /qs/install.sh' > /tmp/install.log 2>&1; then
    echo "PASS  every command of the block succeeded"
else
    echo "FAIL  the block stopped with an error:"; tail -5 /tmp/install.log | sed 's/^/      /'; fails=$((fails + 1))
fi
check "vividctl in a new bash login shell" su - tester -c 'vividctl --version'
check "vividctl in a new interactive bash" su - tester -c 'bash -ic "vividctl --version" 2>/dev/null'
check "vividctl in a new zsh login shell" su - tester -s /usr/bin/zsh -c 'vividctl --version'
check "vividctl list runs (no monitor here)" su - tester -c 'vividctl list'
echo "failures: $fails"
exit "$fails"
EOF

image="vividctl-quickstart:$tag"
docker build -q -t "$image" - >/dev/null <<EOF
FROM ubuntu:$tag
ENV DEBIAN_FRONTEND=noninteractive
# A fresh desktop has package lists, sudo and (for some users) zsh; it does not have git or curl.
RUN apt-get update && apt-get install -y --no-install-recommends sudo zsh ca-certificates \
    && echo 'APT::Get::Assume-Yes "true";' > /etc/apt/apt.conf.d/90assume-yes \
    && useradd -m -s /bin/bash tester && echo 'tester ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/tester
EOF
docker run --rm -v "$work/install.sh:/qs/install.sh:ro" -v "$work/check.sh:/qs/check.sh:ro" \
    -v "$work/repo.git:/repo.git:ro" "$image" bash /qs/check.sh
