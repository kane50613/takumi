#!/bin/sh
# Names and prunes the docs preview aliases under preview.takumi.kane.tw.
#   preview-alias.sh name <branch>  prints the alias a branch deploys to
#   preview-alias.sh prune          removes every branch alias no open pull request deploys to
# prune reads VERCEL_TOKEN, VERCEL_ORG_ID, VERCEL_PROJECT_ID, and gh's GH_TOKEN.
set -eu

domain=preview.takumi.kane.tw

alias_name() {
  # The full alias stays under the 64-character TLS common-name limit.
  ref=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed -E 's#[^a-z0-9-]+#-#g; s/^-+//; s/-{2,}/-/g' | cut -c1-40 | sed -E 's/-+$//')
  echo "${ref:-preview}.$domain"
}

vercel_api() {
  method=$1
  path=$2
  shift 2

  case "$VERCEL_ORG_ID" in
    team_*) team="teamId=$VERCEL_ORG_ID&" ;;
    *) team="" ;;
  esac

  curl -fsS -X "$method" -H "Authorization: Bearer $VERCEL_TOKEN" "https://api.vercel.com$path?$team$*"
}

prune() {
  refs=$(gh pr list --state open --limit 1000 --json headRefName,isCrossRepository \
    -q '.[] | select(.isCrossRepository | not) | .headRefName')
  keep=$(printf '%s\n' "$refs" | while read -r ref; do alias_name "$ref"; done)
  until=""
  removed=0

  while :; do
    page=$(vercel_api GET /v4/aliases "projectId=$VERCEL_PROJECT_ID&limit=100${until:+&until=$until}")

    for entry in $(printf '%s' "$page" | jq -r '.aliases[] | "\(.uid),\(.alias)"'); do
      uid=${entry%%,*}
      name=${entry#*,}

      case "$name" in
        *."$domain") ;;
        *) continue ;;
      esac

      if printf '%s\n' "$keep" | grep -qxF "$name"; then
        continue
      fi

      vercel_api DELETE "/v2/aliases/$uid" >/dev/null
      echo "removed $name"
      removed=$((removed + 1))
    done

    until=$(printf '%s' "$page" | jq -r '.pagination.next // empty')

    if [ -z "$until" ]; then
      break
    fi
  done

  echo "removed $removed aliases"
}

case "${1:-}" in
  name) alias_name "$2" ;;
  prune) prune ;;
  *) echo "usage: $0 name <branch> | prune" >&2; exit 2 ;;
esac
