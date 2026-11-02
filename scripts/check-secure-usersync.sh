#!/bin/sh
# Axum test dev-dependencies enable insecure bidders. Build the production
# graph separately and retain its binary before Cargo builds the test graph.
set -eu
cd "$(dirname "$0")/.."
directory=$(mktemp -d)
trap 'rm -rf "$directory"' EXIT
trap 'exit 1' HUP INT TERM
cargo build --locked -p auctioneer-adapter-axum --bin auctioneer-adapter-axum
cp "${CARGO_TARGET_DIR:-target}/debug/auctioneer-adapter-axum" "$directory/auctioneer-adapter-axum"
selected_test=tests::secure_production_binary_rejects_loopback_callback_config
if AUCTIONEER__TEST__SECURE_AXUM_BINARY="$directory/auctioneer-adapter-axum" \
  cargo test --locked -p auctioneer-adapter-axum --test usersync_transport \
  "$selected_test" -- --exact --ignored --color never > "$directory/test.log" 2>&1; then
  cat "$directory/test.log"
else
  status=$?
  cat "$directory/test.log" >&2
  exit "$status"
fi
if ! grep -Fx "test $selected_test ... ok" "$directory/test.log" > /dev/null; then
  echo "::error::secure user-sync check did not execute and pass $selected_test" >&2
  exit 1
fi
