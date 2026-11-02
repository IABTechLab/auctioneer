#!/usr/bin/env bash
set -euo pipefail
# wrangler prints: `Total Upload: <X> KiB / gzip: <Y> KiB`
report=$(grep -E 'Total Upload: .* KiB / gzip: .* KiB' /tmp/wrangler.log | tail -1 || true)
if [ -z "$report" ]; then
  echo "::error::could not find wrangler's 'Total Upload' report; refusing to pass a gate that measured nothing"
  exit 1
fi
echo "wrangler reported: ${report}"

raw_uncompressed=$(sed -E 's/.*Total Upload: *([0-9.]+) *KiB.*/\1/' <<<"$report")
raw_gzip=$(sed -E 's/.*gzip: *([0-9.]+) *KiB.*/\1/' <<<"$report")

# wrangler reports KiB (binary, 1024 bytes); the ceilings are
# decimal bytes. Convert with the binary factor, compare
# against the decimal budget -- mixing the two is the bug
# this gate is built to avoid.
uncompressed=$(python3 -c "import sys; print(int(float(sys.argv[1]) * 1024))" "$raw_uncompressed")
gzip_bytes=$(python3 -c "import sys; print(int(float(sys.argv[1]) * 1024))" "$raw_gzip")

echo "uncompressed: ${uncompressed} bytes (ceiling ${CF_UNCOMPRESSED_MAX_BYTES})"
echo "gzip:         ${gzip_bytes} bytes (ceiling ${CF_GZIP_MAX_BYTES})"
{
  echo "### Cloudflare upload size"
  echo
  echo "| measure | bytes | ceiling |"
  echo "| --- | ---: | ---: |"
  echo "| uncompressed | ${uncompressed} | ${CF_UNCOMPRESSED_MAX_BYTES} |"
  echo "| gzip | ${gzip_bytes} | ${CF_GZIP_MAX_BYTES} |"
} >> "$GITHUB_STEP_SUMMARY"

status=0
if [ "$uncompressed" -gt "$CF_UNCOMPRESSED_MAX_BYTES" ]; then
  echo "::error::uncompressed upload is ${uncompressed} bytes, over the ${CF_UNCOMPRESSED_MAX_BYTES}-byte budget"
  status=1
fi
if [ "$gzip_bytes" -gt "$CF_GZIP_MAX_BYTES" ]; then
  echo "::error::gzip upload is ${gzip_bytes} bytes, over the ${CF_GZIP_MAX_BYTES}-byte budget"
  status=1
fi
exit "$status"
