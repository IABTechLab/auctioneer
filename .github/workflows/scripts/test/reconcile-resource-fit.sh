#!/usr/bin/env bash
set -euo pipefail
if cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --require-fit > "$RUNNER_TEMP/resource-fit.json"; then
  status=0
else
  status=$?
fi
case "$status" in
  0|1) ;;
  *) exit "$status" ;;
esac
jq -es --argjson status "$status" '
  length == 1 and (.[0] |
    .schema_version == 1
    and .accounting_complete == true
    and .selection == {"scope":"all_adapters"}
    and ([.adapters[].adapter] == ["axum", "cloudflare", "fastly", "spin"])
    and all(.adapters[]; (.required_fit | type) == "boolean")
    and ($status == (if all(.adapters[]; .required_fit == true) then 0 else 1 end))
  )
' "$RUNNER_TEMP/resource-fit.json" > /dev/null
