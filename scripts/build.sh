#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$root/web"
pnpm install --frozen-lockfile
pnpm build

cd "$root"
cargo build --release -p selena-cli
cargo build --release -p selena-desktop

echo "Binaries: target/release/selena and target/release/selena-desktop"
