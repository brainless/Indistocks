#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored messages
error() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

success() {
    echo -e "${GREEN}$1${NC}"
}

warning() {
    echo -e "${YELLOW}$1${NC}"
}

info() {
    echo "$1"
}

# Check if we're in the project root
if [ ! -f "Cargo.toml" ] || [ ! -d "indistocks-gui" ]; then
    error "Must be run from project root directory"
fi

# Check if we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    error "Releases can only be created from main branch. Current branch: $CURRENT_BRANCH"
fi

# Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
    error "Working directory is not clean. Commit or stash changes before releasing."
fi

# Parse command line arguments
NEW_VERSION=""
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--version)
            NEW_VERSION="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [-v|--version VERSION]"
            echo ""
            echo "Options:"
            echo "  -v, --version VERSION  Set specific version (e.g., 0.2.0)"
            echo "  -h, --help            Show this help message"
            echo ""
            echo "If no version is specified, the minor version will be incremented."
            exit 0
            ;;
        *)
            error "Unknown option: $1. Use -h for help."
            ;;
    esac
done

# Get current version from indistocks-gui/Cargo.toml
CURRENT_VERSION=$(grep '^version = ' indistocks-gui/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ -z "$CURRENT_VERSION" ]; then
    error "Could not determine current version from indistocks-gui/Cargo.toml"
fi

info "Current version: $CURRENT_VERSION"

# Calculate new version if not provided
if [ -z "$NEW_VERSION" ]; then
    # Split version into major.minor.patch
    IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
    MAJOR="${VERSION_PARTS[0]}"
    MINOR="${VERSION_PARTS[1]}"
    PATCH="${VERSION_PARTS[2]}"

    # Increment minor version
    MINOR=$((MINOR + 1))
    NEW_VERSION="$MAJOR.$MINOR.0"
fi

# Validate version format
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    error "Invalid version format: $NEW_VERSION (expected: X.Y.Z)"
fi

# Check if version is greater than current
if [ "$NEW_VERSION" = "$CURRENT_VERSION" ]; then
    error "New version ($NEW_VERSION) must be different from current version ($CURRENT_VERSION)"
fi

info "New version: $NEW_VERSION"

# Check if tag already exists
if git rev-parse "v$NEW_VERSION" >/dev/null 2>&1; then
    error "Tag v$NEW_VERSION already exists"
fi

# Generate changelog from commits since last tag
PREV_TAG=$(git tag --sort=-v:refname | head -n 1)

if [ -z "$PREV_TAG" ]; then
    info "No previous tags found. This is the first release."
    COMMITS=$(git log --pretty=format:"- %s (%h)")
else
    info "Generating changelog from $PREV_TAG to HEAD"
    COMMITS=$(git log --pretty=format:"- %s (%h)" $PREV_TAG..HEAD)
fi

if [ -z "$COMMITS" ]; then
    error "No commits found since last release. Nothing to release."
fi

# Display the changelog
warning "\nChangelog:"
echo "$COMMITS"
echo ""

# Confirm release
read -p "Create release v$NEW_VERSION? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    info "Release cancelled."
    exit 0
fi

# Update version in Cargo.toml files
info "\nUpdating version in Cargo.toml files..."

# Update indistocks-gui/Cargo.toml
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" indistocks-gui/Cargo.toml

# Update indistocks-db/Cargo.toml if it has a version field
if grep -q '^version = ' indistocks-db/Cargo.toml 2>/dev/null; then
    sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" indistocks-db/Cargo.toml
fi

# Update Cargo.lock
info "Updating Cargo.lock..."
cargo update --workspace --quiet 2>/dev/null || cargo check --quiet

# Commit version changes
info "Committing version changes..."
git add Cargo.toml indistocks-gui/Cargo.toml indistocks-db/Cargo.toml Cargo.lock 2>/dev/null || true
git commit -m "Bump version to $NEW_VERSION"

# Create annotated tag
info "Creating tag v$NEW_VERSION..."
TAG_MESSAGE="Release $NEW_VERSION

$COMMITS"

git tag -a "v$NEW_VERSION" -m "$TAG_MESSAGE"

success "\nRelease v$NEW_VERSION created successfully!"
info "\nTo publish the release, run:"
info "  git push origin main --tags"
info "\nThis will trigger the GitHub Actions workflow to build and publish the release."
