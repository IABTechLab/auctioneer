#!/usr/bin/env bash
set -euo pipefail
version="$(awk '$1 == "wasmtime" { print $2 }' .tool-versions)"
curl --fail --location --retry 3 --max-time 60 \
  --output "$RUNNER_TEMP/wasmtime.tar.xz" \
  "https://github.com/bytecodealliance/wasmtime/releases/download/v${version}/wasmtime-v${version}-x86_64-linux.tar.xz"
echo "f2b0ad1ce9253f2f9a38793c2c42cd1cba4e90b27dc40d685eaf723dc8438d94  $RUNNER_TEMP/wasmtime.tar.xz" | sha256sum --check
mkdir -p "$RUNNER_TEMP/schema-guest-tools"
tar -xJf "$RUNNER_TEMP/wasmtime.tar.xz" -C "$RUNNER_TEMP/schema-guest-tools" --strip-components=1
echo "$RUNNER_TEMP/schema-guest-tools" >> "$GITHUB_PATH"
test "$("$RUNNER_TEMP/schema-guest-tools/wasmtime" --version | awk '{ print $2 }')" = "$version"
