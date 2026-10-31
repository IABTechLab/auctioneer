#!/usr/bin/env bash
set -euo pipefail
cargo test -p auctioneer-adapter-fastly --lib --target wasm32-wasip1 \
  --no-run -j2 --message-format=json > "$RUNNER_TEMP/fastly-library-tests.json"
wasm=$(jq -ers '
  [.[] | select(.reason == "compiler-artifact"
    and .target.name == "auctioneer_adapter_fastly"
    and (.target.kind | index("lib")) != null
    and .profile.test)
    | .executable | select(. != null and endswith(".wasm"))]
  | unique
  | if length == 1 then .[0]
    else error("expected exactly one Fastly library-test WASM artifact") end
' "$RUNNER_TEMP/fastly-library-tests.json")
test -f "$wasm"
echo "wasm=$wasm" >> "$GITHUB_OUTPUT"
