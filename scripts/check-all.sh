#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if command -v pnpm >/dev/null 2>&1; then
  PNPM=(pnpm)
else
  PNPM=(corepack pnpm)
fi

echo "==> [1/9] Generating Rust -> TS bindings and checking zero diff"
"${PNPM[@]}" gen:bindings
git diff --exit-code src/bindings

echo "==> [2/9] Running static architecture checks"
node scripts/check-architecture.mjs

echo "==> [3/9] Checking Prettier formatting"
"${PNPM[@]}" format:check

echo "==> [4/9] Running ESLint"
"${PNPM[@]}" lint

echo "==> [5/9] Running TypeScript / vue-tsc typecheck"
"${PNPM[@]}" typecheck

echo "==> [6/9] Running frontend unit tests (Vitest)"
"${PNPM[@]}" test:unit --run

echo "==> [7/9] Checking Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

echo "==> [8/9] Running Rust Clippy (-D warnings)"
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

echo "==> [9/9] Running Rust tests"
cargo test --manifest-path src-tauri/Cargo.toml --all-features

echo "All 9 gate checks passed!"
