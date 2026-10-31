#!/usr/bin/env bash
set -euo pipefail
if [ "$AFTER_EPOCH" -gt "$BEFORE_EPOCH" ]; then
  echo "advanced=true" >> "$GITHUB_OUTPUT"
else
  echo "advanced=false" >> "$GITHUB_OUTPUT"
  # Drop the restamped `generated_at` (and anything else the
  # run touched) so the checkout is clean and no branch,
  # commit or pull request is created from a no-op.
  git checkout -- .
fi
