#!/usr/bin/env bash
set -euo pipefail
branch="refresh-gvl/${AFTER_EXPIRY}"
branch="${branch//:/-}"
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
# The branch name is derived from the expiry, so a retry after a
# partial failure (pushed, then PR creation failed) targets the
# SAME branch. A plain push would then fail non-fast-forward
# before ever reaching the PR step, wedging the automation until
# someone deletes the branch by hand. The branch is owned by this
# job and its content is a deterministic function of the refresh,
# so force-with-lease against whatever a previous attempt left is
# the idempotent choice: any retry converges on the same tree.
git checkout -B "$branch"
git add crates/auctioneer-config/data/gvl_archive
git commit -m "chore: refresh the baked GVL and CMP archive" \
  -m "The baked Global CMP List expired at ${BEFORE_EXPIRY}; the refreshed artifact expires at ${AFTER_EXPIRY}."
git push --force-with-lease --set-upstream origin "$branch"
body=$(printf '%s\n' \
  "Automated refresh of \`crates/auctioneer-config/data/gvl_archive/\`," \
  "produced by \`cargo run --locked -p auctioneer-cli --no-default-features -- gvl refresh\`. The job runs daily" \
  "and opens a pull request only on the days the Global CMP List expiry" \
  "actually moved forward." \
  "" \
  "**Expiry:** \`${BEFORE_EXPIRY}\` to \`${AFTER_EXPIRY}\`. Past that instant" \
  "the build gate rejects the archive and every TCF request takes the" \
  "stale-GCL fail-closed path (warning 10019), so this refresh is what" \
  "keeps enforcement running." \
  "" \
  "**New expiry derivation:** \`gcl.fetched_at\` \`${AFTER_FETCHED}\` plus" \
  "\`gcl.cache_max_age_secs\` \`${AFTER_MAX_AGE}\`." \
  "" \
  "**Upstream list timestamp:** \`${BEFORE_LIST}\` to \`${AFTER_LIST}\`." \
  "" \
  "**Files changed in the archive:**" \
  '```' \
  "${CHANGED_FILES}" \
  '```')
# Create-or-update: a retry whose previous attempt already opened
# the PR must refresh its body, not die on "a pull request
# already exists".
existing=$(gh pr list --head "$branch" --state open --json number --jq '.[0].number // empty')
if [ -n "$existing" ]; then
  gh pr edit "$existing" \
    --title "Refresh the baked GVL/CMP archive (expires ${AFTER_EXPIRY})" \
    --body "$body"
else
  gh pr create \
    --title "Refresh the baked GVL/CMP archive (expires ${AFTER_EXPIRY})" \
    --body "$body"
fi
