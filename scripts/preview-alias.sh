#!/bin/sh
# Prints the docs preview alias a branch deploys to, under preview.takumi.kane.tw.
set -eu

# The full alias stays under the 64-character TLS common-name limit.
ref=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed -E 's#[^a-z0-9-]+#-#g; s/^-+//; s/-{2,}/-/g' | cut -c1-40 | sed -E 's/-+$//')
echo "${ref:-preview}.preview.takumi.kane.tw"
