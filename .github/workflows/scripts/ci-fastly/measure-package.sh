#!/usr/bin/env bash
set -euo pipefail
package=crates/auctioneer-adapter-fastly/pkg/auctioneer.tar.gz
if [ ! -f "$package" ]; then
  echo "::error::fastly compute build did not produce $package"
  exit 1
fi
bytes=$(wc -c < "$package" | tr -d '[:space:]')
echo "fastly package: ${bytes} bytes (ceiling ${FASTLY_PACKAGE_MAX_BYTES})"
echo "bytes=${bytes}" >> "$GITHUB_OUTPUT"
{
  echo "### Fastly artifact size"
  echo
  echo "| package | bytes | ceiling |"
  echo "| --- | ---: | ---: |"
  echo "| \`auctioneer.tar.gz\` | ${bytes} | ${FASTLY_PACKAGE_MAX_BYTES} |"
} >> "$GITHUB_STEP_SUMMARY"
if [ "$bytes" -gt "$FASTLY_PACKAGE_MAX_BYTES" ]; then
  echo "::error::fastly package is ${bytes} bytes, over the ${FASTLY_PACKAGE_MAX_BYTES}-byte ceiling"
  exit 1
fi
