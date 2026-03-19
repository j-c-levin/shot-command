#!/bin/bash
# Auto-increment patch version, tag, and push.
# Usage: ./scripts/release.sh        (bumps patch: 0.1.0 -> 0.1.1)
#        ./scripts/release.sh minor  (bumps minor: 0.1.1 -> 0.2.0)
#        ./scripts/release.sh major  (bumps major: 0.2.0 -> 1.0.0)

set -e

BUMP=${1:-patch}

# Get latest version tag
LATEST=$(git tag -l 'v*' --sort=-v:refname | head -1)
if [ -z "$LATEST" ]; then
  LATEST="v0.0.0"
fi

# Parse version
VERSION=${LATEST#v}
IFS='.' read -r MAJOR MINOR PATCH <<< "$VERSION"

case $BUMP in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  patch) PATCH=$((PATCH + 1)) ;;
  *) echo "Usage: $0 [major|minor|patch]"; exit 1 ;;
esac

NEW_VERSION="v${MAJOR}.${MINOR}.${PATCH}"
echo "Releasing: $LATEST -> $NEW_VERSION"

# Update Cargo.toml version
sed -i '' "s/^version = \".*\"/version = \"${MAJOR}.${MINOR}.${PATCH}\"/" Cargo.toml

# Commit, tag, push
git add Cargo.toml
git -c commit.gpgsign=false commit -m "release: ${NEW_VERSION}"
git tag "$NEW_VERSION"
git push origin main
git push origin "$NEW_VERSION"

echo "Done! Pipeline triggered for $NEW_VERSION"
echo "Watch: https://github.com/j-c-levin/shot-command/actions"
