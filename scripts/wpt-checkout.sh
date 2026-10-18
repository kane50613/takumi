#!/bin/sh
# Checks out the web-platform-tests that takumi/tests/wpt_reftests.rs runs into target/wpt, at the
# revision Chromium last imported, so local runs and CI test against the same files.
set -eu

revision=eed8aea45725da5b002ee8778400d14148d0ebfe
dir="$(cd "$(dirname "$0")/.." && pwd)/target/wpt"

if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null)" = "$revision" ]; then
  exit 0
fi

mkdir -p "$dir"
git -C "$dir" init -q
git -C "$dir" remote remove origin 2>/dev/null || true
git -C "$dir" remote add origin https://github.com/web-platform-tests/wpt.git
git -C "$dir" sparse-checkout set \
  css/css-text css/css-text-decor css/css-inline css/css-overflow \
  css/CSS2/linebox css/CSS2/text css/CSS2/generated-content \
  css/reference css/support css/CSS2/reference css/CSS2/support \
  common fonts images
git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$revision"
git -C "$dir" checkout -q --force FETCH_HEAD
