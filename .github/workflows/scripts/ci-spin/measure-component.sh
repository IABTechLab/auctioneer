#!/usr/bin/env bash
set -euo pipefail
component=target/wasm32-wasip2/wasm-release/auctioneer_adapter_spin.wasm
if [ ! -f "$component" ]; then
  echo "::error::spin build did not produce $component"
  exit 1
fi
bytes=$(wc -c < "$component" | tr -d '[:space:]')
# Printed either way, pass or fail, so the trend is visible
# in the log even when the gate is green.
echo "spin component: ${bytes} bytes (ceiling ${SPIN_COMPONENT_MAX_BYTES})"
echo "bytes=${bytes}" >> "$GITHUB_OUTPUT"
{
  echo "### Spin artifact size"
  echo
  echo "| component | bytes | ceiling |"
  echo "| --- | ---: | ---: |"
  echo "| \`auctioneer_adapter_spin.wasm\` | ${bytes} | ${SPIN_COMPONENT_MAX_BYTES} |"
} >> "$GITHUB_STEP_SUMMARY"
if [ "$bytes" -gt "$SPIN_COMPONENT_MAX_BYTES" ]; then
  echo "::error::spin component is ${bytes} bytes, over the ${SPIN_COMPONENT_MAX_BYTES}-byte budget"
  exit 1
fi
