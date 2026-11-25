#!/usr/bin/env bash
# Pushes the current branch to GitHub using a token from the environment.
#
# The token is read from $GITHUB_TOKEN at run time and handed to git through a
# one-shot credential helper, so it is never written to .git/config, to a file,
# or to the remote URL. In Freebuff Cloud the project's synced secrets already
# arrive in the environment, so no extra setup is needed once the secret exists.
#
#   GITHUB_TOKEN=... scripts/publish.sh            # push branch -> main
#   GITHUB_TOKEN=... scripts/publish.sh origin dev # push branch -> dev
#
# Afterwards, tag a release so .github/workflows/release.yml builds the
# Windows installer:
#
#   git tag v1.0.0 && git push origin v1.0.0

set -euo pipefail

if [ -z "${GITHUB_TOKEN:-}" ]; then
  echo "GITHUB_TOKEN is not set." >&2
  echo "Add a fine-grained PAT (Contents: Read and write) as the project secret" >&2
  echo "GITHUB_TOKEN, or export it for this one command." >&2
  exit 1
fi

remote="${1:-origin}"
target="${2:-main}"
branch="$(git rev-parse --abbrev-ref HEAD)"

if [ "$branch" = "HEAD" ]; then
  echo "Refusing to publish a detached HEAD; check out a branch first." >&2
  exit 1
fi

echo "Pushing $branch -> $remote/$target ..."
git -c credential.helper= \
    -c 'credential.helper=!f() { test "$1" = get && printf "username=x-access-token\npassword=%s\n" "$GITHUB_TOKEN"; }; f' \
    push "$remote" "$branch:$target"
