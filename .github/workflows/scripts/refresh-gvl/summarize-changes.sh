#!/usr/bin/env bash
set -euo pipefail
{
  echo "files<<ARCHIVE_EOF"
  git status --porcelain -- crates/auctioneer-config-data/data/gvl_archive
  echo "ARCHIVE_EOF"
} >> "$GITHUB_OUTPUT"
