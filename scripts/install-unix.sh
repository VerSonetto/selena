#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${PREFIX:-/usr/local}"

cd "$root"
cargo build --release -p selena-cli -p selena-desktop

install -d "$prefix/bin"
install -m 755 target/release/selena "$prefix/bin/selena"
install -m 755 target/release/selena-desktop "$prefix/bin/selena-desktop"

# Shell lookup is case-sensitive on most Unix filesystems. Install the aliases
# so SELENA and Selena resolve to the same binary as selena.
ln -sf selena "$prefix/bin/Selena"
ln -sf selena "$prefix/bin/SELENA"

echo "Installed to $prefix/bin"
echo "Try: selena --version && SELENA WEB"
