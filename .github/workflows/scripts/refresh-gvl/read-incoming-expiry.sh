#!/usr/bin/env bash
set -euo pipefail
manifest=crates/auctioneer-config/data/gvl_archive/manifest.json
fetched=$(jq -r '.gcl.fetched_at' "$manifest")
max_age=$(jq -r '.gcl.cache_max_age_secs' "$manifest")
expiry_epoch=$(( $(date -u -d "$fetched" +%s) + max_age ))
{
  echo "expiry_epoch=$expiry_epoch"
  echo "expiry=$(date -u -d "@$expiry_epoch" +"%Y-%m-%dT%H:%M:%SZ")"
  echo "fetched_at=$fetched"
  echo "max_age=$max_age"
  echo "list_version=$(jq -r '.gcl.last_updated' "$manifest")"
} >> "$GITHUB_OUTPUT"
