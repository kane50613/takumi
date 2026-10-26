#!/bin/sh
# Checks out the web-platform-tests that takumi/tests/wpt_reftests.rs runs into $WPT_ROOT, or
# target/wpt, at the revision Chromium last imported, so local runs and CI test against the same files.
set -eu

revision=eed8aea45725da5b002ee8778400d14148d0ebfe
dir="${WPT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)/target/wpt}"
# The CSS modules and SVG tests takumi renders, then what they share.
paths="
  css/css-text css/css-text-decor css/css-inline css/css-overflow
  css/css-backgrounds css/css-box css/css-break css/css-color css/css-display css/css-flexbox css/css-fonts
  css/css-grid css/css-images css/css-masking css/css-position css/css-sizing css/css-tables
  css/css-transforms css/compositing css/filter-effects
  css/CSS2/backgrounds css/CSS2/borders css/CSS2/box css/CSS2/colors css/CSS2/floats
  css/CSS2/generated-content css/CSS2/linebox css/CSS2/lists css/CSS2/margin-padding-clear
  css/CSS2/normal-flow css/CSS2/positioning css/CSS2/tables css/CSS2/text css/CSS2/visudet
  css/CSS2/visuren
  css/reference css/support css/CSS2/reference css/CSS2/support
  svg common fonts images
"
stamp="$dir/.takumi-paths"

if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null)" = "$revision" ] \
  && [ "$(cat "$stamp" 2>/dev/null)" = "$(printf '%s' "$paths")" ]; then
  exit 0
fi

mkdir -p "$dir"
git -C "$dir" init -q
git -C "$dir" remote remove origin 2>/dev/null || true
git -C "$dir" remote add origin https://github.com/web-platform-tests/wpt.git
# Re-adding the remote drops the partial clone settings, without which Git cannot fetch the blobs
# a wider path list needs.
git -C "$dir" config remote.origin.promisor true
git -C "$dir" config remote.origin.partialclonefilter blob:none
# shellcheck disable=SC2086 # The paths split on purpose.
git -C "$dir" sparse-checkout set $paths
git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$revision"
git -C "$dir" checkout -q --force FETCH_HEAD
test -d "$dir/css/css-inline" || { echo "wpt-checkout: $dir/css is missing after checkout" >&2; exit 1; }
printf '%s' "$paths" > "$stamp"
