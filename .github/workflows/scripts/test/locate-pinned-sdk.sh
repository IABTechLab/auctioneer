#!/usr/bin/env bash
set -euo pipefail
pin=775366fdac923135e8430b1e3dffebedc648ef7c
cargo metadata --locked --offline --format-version 1 > "$RUNNER_TEMP/qualification-metadata.json"
manifest=$(jq -er --arg source "git+https://github.com/IABTechLab/edgezero.git?rev=$pin#$pin" '
  [.packages[] | select(.name == "edgezero-cli")]
  | if length == 1 and .[0].source == $source
      and (.[0].manifest_path | type == "string")
      and (.[0].manifest_path | startswith("/"))
      and (.[0].manifest_path | test("[\\r\\n]") | not)
    then .[0].manifest_path
    else error("expected exactly one pinned edgezero-cli dependency") end
' "$RUNNER_TEMP/qualification-metadata.json")
/usr/bin/git --no-replace-objects -C "$(dirname -- "$manifest")" rev-parse --show-toplevel > "$RUNNER_TEMP/qualification-sdk-root"
root=$(jq -erRs '
  if endswith("\n") then .[:-1] else . end
  | if startswith("/") and (test("[\\r\\n]") | not)
    then . else error("invalid SDK checkout path") end
' "$RUNNER_TEMP/qualification-sdk-root")
test "$(/usr/bin/git --no-replace-objects -C "$root" rev-parse HEAD)" = "$pin"
printf 'EDGEZERO_SDK_ROOT=%s\n' "$root" >> "$GITHUB_ENV"
