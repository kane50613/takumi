#!/bin/sh
# Appends the raw, gzip, and brotli sizes of a wasm build to the job summary, with the change from
# the same artifact of the PR base branch's latest successful run.
# Usage: report-wasm-size.sh <label> <wasm path> <artifact name>
set -eu

label=$1
wasm=$2
artifact=$3

raw=$(wc -c < "$wasm")
gz=$(gzip -9 -c "$wasm" | wc -c)
br=$(brotli -q 11 -c "$wasm" | wc -c)
base_raw=""
base_gz=""
base_br=""

if [ -n "${BASE_REF:-}" ]; then
  base="$RUNNER_TEMP/base-$artifact"
  run=$(gh run list -R "$GITHUB_REPOSITORY" --workflow ci.yml --branch "$BASE_REF" --status success -L 1 --json databaseId -q '.[0].databaseId' || true)

  if [ -n "$run" ] && gh run download "$run" -R "$GITHUB_REPOSITORY" -n "$artifact" -D "$base" > /dev/null 2>&1 && [ -f "$base/$wasm" ]; then
    base_raw=$(wc -c < "$base/$wasm")
    base_gz=$(gzip -9 -c "$base/$wasm" | wc -c)
    base_br=$(brotli -q 11 -c "$base/$wasm" | wc -c)
  fi
fi

{
  echo "| $label | bytes | MiB | change from \`${BASE_REF:-base}\` |"
  echo "| --- | ---: | ---: | ---: |"
  awk -v raw="$raw" -v gz="$gz" -v br="$br" -v base_raw="$base_raw" -v base_gz="$base_gz" -v base_br="$base_br" 'BEGIN {
    printf "| raw | %d | %.2f | %s |\n", raw, raw / 1048576, delta(raw, base_raw)
    printf "| gzip | %d | %.2f | %s |\n", gz, gz / 1048576, delta(gz, base_gz)
    printf "| brotli | %d | %.2f | %s |\n", br, br / 1048576, delta(br, base_br)
  }
  function delta(now, before) {
    if (before == "") return "n/a"
    return sprintf("%+d (%+.2f%%)", now - before, 100 * (now - before) / before)
  }'
} >> "$GITHUB_STEP_SUMMARY"
