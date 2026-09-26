#!/usr/bin/env bash
# Gathers what a failed test run left for review, for upload as artifacts:
# - snapshots.tar: every missing or changed snapshot and capture, and the
#   diffs, at their paths in the repository, which
#   `.github/scripts/accept-snapshots.sh <run id>` accepts;
# - visual-review/: `cargo mitsuami visual review`'s static report of the
#   changed captures, if there are any, noted in the job's summary.
# Usage: collect-review.sh <artifact suffix>
set -euo pipefail

# The archive path is relative: GNU tar reads `D:\…` as a remote host.
find crates -path '*/tests/*' \( -name '*.new' -o -name '*.new.png' -o -name '*.diff.png' \) -print0 \
  | tar --null -cf snapshots.tar -T -

count=$(find crates -path '*/tests/visual/*' -name '*.new.png' | wc -l | tr -d ' ')
if [ "$count" -gt 0 ]; then
  cargo mitsuami visual review --out visual-review
  cat >> "$GITHUB_STEP_SUMMARY" <<EOF
### $count captures to review ($1)

Download the \`visual-review-$1\` artifact and open \`index.html\`. To accept them all:
\`.github/scripts/accept-snapshots.sh $GITHUB_RUN_ID\`
EOF
fi
