#!/bin/sh
# Builds the docs and deploys them to Vercel from CI, then writes the preview alias to $GITHUB_OUTPUT.
# A pull request deploys a preview to its branch alias. A push to master deploys production, or a
# preview at preview.takumi.kane.tw while a pending changeset means the release PR is still open,
# so production keeps the docs of the published release.
set -eu

if [ "$GITHUB_EVENT_NAME" = pull_request ]; then
  alias=$(.github/scripts/preview-alias.sh "$GITHUB_HEAD_REF")
elif find .tegami -maxdepth 1 -name '*.md' | grep -q .; then
  alias=preview.takumi.kane.tw
else
  alias=""
fi

vercel() {
  bunx vercel "$@" --token="$VERCEL_TOKEN"
}

if [ -z "$alias" ]; then
  vercel pull --yes --environment=production
  vercel build --prod
  rm -rf .vercel/output/functions
  vercel deploy --archive=tgz --prebuilt --prod
  exit
fi

vercel pull --yes --environment=preview
vercel build
rm -rf .vercel/output/functions

# Preview aliases sit on a custom domain, which Vercel does not mark noindex itself. robots.txt
# stays as it is, since a crawler it turns away never reads the header.
jq '.routes = [{"src": "/(.*)", "headers": {"X-Robots-Tag": "noindex"}, "continue": true}] + (.routes // [])' \
  .vercel/output/config.json > .vercel/output/config.json.tmp
mv .vercel/output/config.json.tmp .vercel/output/config.json

url=$(vercel deploy --archive=tgz --prebuilt)
vercel alias set "$url" "$alias"
echo "alias=$alias" >> "$GITHUB_OUTPUT"
