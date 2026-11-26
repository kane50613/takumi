#!/usr/bin/env bash
# Reports a wasm package's raw, gzip and brotli sizes to the step summary and enforces .github/wasm-size-budget.json.
#
# usage: wasm-size.sh <package> <wasm> [named-wasm]
#
# <package> is a key in the budget file. <wasm> and [named-wasm] are `<artifact>/<path>` paths relative to the
# current directory, which holds this run's artifacts. A pull request downloads the same artifacts from its base
# branch's latest successful run and compares against them. [named-wasm] is a build that keeps the name section,
# which twiggy needs to attribute the change to functions.
#
# A pull request that grows brotli past the budget fails unless it carries the `size-increase` label.
set -euo pipefail

package=$1
wasm=$2
named=${3:-}
budget_file="$(dirname "$0")/../wasm-size-budget.json"
override_label=size-increase
summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}
work=$(mktemp -d)

sizes() {
  echo "$(wc -c < "$1") $(gzip -9 -c "$1" | wc -c) $(brotli -q 11 -c "$1" | wc -c)"
}

base_run=""
if [ -n "${BASE_REF:-}" ]; then
  base_run=$(gh run list -R "$GITHUB_REPOSITORY" --workflow ci.yml --branch "$BASE_REF" --status success -L 1 \
    --json databaseId -q '.[0].databaseId' || true)
fi

# Prints the base run's copy of an `<artifact>/<path>` file, or nothing when the base run has no such artifact.
fetch_base() {
  local artifact=${1%%/*}

  if [ -n "$base_run" ] && [ ! -d "$work/base/$artifact" ]; then
    gh run download "$base_run" -R "$GITHUB_REPOSITORY" -n "$artifact" -D "$work/base/$artifact" > /dev/null 2>&1 || true
  fi

  if [ -f "$work/base/$1" ]; then
    echo "$work/base/$1"
  fi
}

# Reads the labels live so that re-running the job sees a label added after the run started.
has_override_label() {
  local labels

  [ -n "${PR_NUMBER:-}" ] || return 1
  labels=$(gh api --paginate "repos/$GITHUB_REPOSITORY/issues/$PR_NUMBER/labels" --jq '.[].name') || return 1
  grep -qx "$override_label" <<< "$labels"
}

# Drops what changes between builds of the same code from twiggy's names: crate hashes and the `.N` suffix
# wasm-opt gives duplicate names.
stable_names() {
  sed -E '/::/{
    s/([A-Za-z0-9_])\[[0-9a-f]{6,16}\]/\1/g
    s/\.[0-9]+$//
  }'
}

# Sums twiggy's per-item sizes by stable name. Unnamed functions, data segments, types and globals are each one
# row, and the name section is left out because it does not ship.
items() {
  twiggy top -n 4294967295 "$1" | stable_names | awk -F'┊' '
    NF < 3 || $NF ~ /^ (Σ \[|\.\.\. and )/ { next }
    {
      bytes = $1 + 0
      name = substr($NF, 2)
      if (name ~ /" subsection$/) next
      if (name ~ /^code\[[0-9]+\]$/) name = "(unnamed functions)"
      else if (name ~ /^data segment /) name = "(data segments)"
      else if (name ~ /^type\[/) name = "(types)"
      else if (name ~ /^global\[/) name = "(globals)"
      sum[name] += bytes
    }
    END { for (name in sum) printf "%d\t%s\n", sum[name], name }'
}

# Prints each generic family's total bytes across its monomorphizations.
families() {
  twiggy monos -g --all-generics "$1" | stable_names | awk -F'┊' '
    NF < 5 || $NF ~ /^ (Σ \[|\.\.\. and )/ { next }
    { sum[substr($NF, 2)] += $3 }
    END { for (name in sum) printf "%d\t%s\n", sum[name], name }'
}

# Joins two `bytes<TAB>name` lists into `delta<TAB>before<TAB>after<TAB>name` rows, largest change first.
join_sizes() {
  awk -F'\t' '
    NR == FNR { before[$2] = $1; names[$2] = 1; next }
    { after[$2] = $1; names[$2] = 1 }
    END {
      for (name in names) {
        delta = after[name] - before[name]
        if (delta != 0) printf "%d\t%d\t%d\t%d\t%s\n", (delta < 0 ? -delta : delta), delta, before[name], after[name], name
      }
    }' "$1" "$2" | sort -t$'\t' -k1,1nr | cut -f2-
}

# Turns `delta[<TAB>bytes...]<TAB>name` rows into Markdown table rows.
markdown_rows() {
  awk -F'\t' '{
    name = $NF
    gsub(/\|/, "\\|", name)
    gsub(/`/, "'"'"'", name)
    if (length(name) > 160) name = substr(name, 1, 157) "..."
    row = sprintf("| %+d |", $1)
    for (i = 2; i < NF; i++) row = row sprintf(" %d |", $i)
    print row " `" name "` |"
  }'
}

read -r raw gz br <<< "$(sizes "$wasm")"
base_wasm=$(fetch_base "$wasm")
base_raw="" base_gz="" base_br=""
if [ -n "$base_wasm" ]; then
  read -r base_raw base_gz base_br <<< "$(sizes "$base_wasm")"
fi

{
  echo "### $package"
  echo
  echo "| | bytes | MiB | change from \`${BASE_REF:-base}\` |"
  echo "| --- | ---: | ---: | ---: |"
  awk -v raw="$raw" -v gz="$gz" -v br="$br" -v base_raw="$base_raw" -v base_gz="$base_gz" -v base_br="$base_br" '
    function row(label, now, before, change) {
      change = "n/a"
      if (before != "") change = sprintf("%+d (%+.2f%%)", now - before, 100 * (now - before) / before)
      printf "| %s | %d | %.2f | %s |\n", label, now, now / 1048576, change
    }
    BEGIN {
      row("raw", raw, base_raw)
      row("gzip -9", gz, base_gz)
      row("brotli -q 11", br, base_br)
    }'
  echo
} >> "$summary"

failed=0
growth=$((br - ${base_br:-$br}))
growth_budget=$(jq -r --arg package "$package" '.[$package].brotliGrowthBytes // empty' "$budget_file")
gzip_ceiling=$(jq -r --arg package "$package" '.[$package].gzipCeilingBytes // empty' "$budget_file")

if [ -n "$gzip_ceiling" ] && [ "$gz" -gt "$gzip_ceiling" ]; then
  echo "::error::$package is $gz bytes gzipped, over its $gzip_ceiling byte ceiling in .github/wasm-size-budget.json."
  echo "The gzip size is over its $gzip_ceiling byte ceiling. The \`$override_label\` label does not lift this ceiling." >> "$summary"
  failed=1
fi

if [ -n "$growth_budget" ] && [ "$growth" -gt "$growth_budget" ]; then
  if has_override_label; then
    echo "::warning::$package grew by $growth bytes brotli, over the $growth_budget byte budget. The $override_label label accepts it."
    echo "Brotli grew by $growth bytes, over the $growth_budget byte budget. The \`$override_label\` label accepts the growth." >> "$summary"
  else
    echo "::error::$package grew by $growth bytes brotli, over the $growth_budget byte budget. If the growth is intended, add the $override_label label to the pull request and re-run this job."
    {
      echo "Brotli grew by $growth bytes, over the $growth_budget byte budget in \`.github/wasm-size-budget.json\`."
      echo "If the growth is intended, add the \`$override_label\` label to the pull request and re-run this job."
    } >> "$summary"
    failed=1
  fi
fi

base_named=""
if [ -n "$named" ]; then
  base_named=$(fetch_base "$named")
fi

if [ -n "$base_named" ]; then
  items "$base_named" > "$work/base-items"
  items "$named" > "$work/items"
  families "$base_named" > "$work/base-families"
  families "$named" > "$work/families"
  join_sizes "$work/base-items" "$work/items" > "$work/item-changes"
  join_sizes "$work/base-families" "$work/families" | awk -F'\t' '$1 > 0' > "$work/family-growth"
  {
    echo
    echo "#### Largest changes by item, in raw bytes"
    echo
    echo "| change | item |"
    echo "| ---: | --- |"
    head -n 20 "$work/item-changes" | cut -f1,4 | markdown_rows
    awk -F'\t' '{ total += $1 } END { printf "| %+d | total of %d changed items |\n", total, NR }' "$work/item-changes"
    echo
    echo "#### Generic families that grew, in raw bytes"
    echo
    if [ -s "$work/family-growth" ]; then
      echo "| change | before | after | generic |"
      echo "| ---: | ---: | ---: | --- |"
      head -n 20 "$work/family-growth" | markdown_rows
    else
      echo "No generic family grew."
    fi
    echo
  } >> "$summary"
fi

exit "$failed"
