#!/bin/bash

# Manual sync script for syncing with upstream

set -e

echo "Starting upstream sync..."

# Ensure we're in the main repository
cd /Users/ningxueye/Developer/github.com/laipz8200/xlaude

# Fetch latest from upstream
echo "Fetching upstream changes..."
git fetch upstream

# Update main branch
echo "Updating main branch..."
git checkout main
git merge upstream/main --no-edit
git push origin main

# Rebase main-custom on updated main
echo "Rebasing main-custom..."
git checkout main-custom
if git rebase main; then
    echo "Rebase successful, pushing to origin..."
    git push origin main-custom --force-with-lease
    echo "✅ Sync completed successfully!"
else
    echo "⚠️  Rebase encountered conflicts. Please resolve manually."
    echo "After resolving conflicts:"
    echo "  1. git rebase --continue"
    echo "  2. git push origin main-custom --force-with-lease"
fi