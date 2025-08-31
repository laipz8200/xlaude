# Fork Maintenance Strategy

This fork maintains custom features not accepted upstream while staying synchronized with the original repository.

## Branch Structure

- **`main`** - Clean mirror of upstream/main, no custom changes
- **`main-custom`** - Default branch with all custom features (currently includes vim navigation)
- **Feature branches** - Individual features that may be submitted upstream

## Custom Features

### Vim Navigation in Dashboard (gg/G)
- **Branch**: `feat/support-gg-and-G-in-dashboard-list`
- **Description**: Adds vim-style `gg` (jump to top) and `G` (jump to bottom) navigation in the dashboard list
- **Status**: Rejected upstream, maintained in fork

## Syncing with Upstream

### Automatic Sync (GitHub Actions)
The repository automatically syncs with upstream weekly via GitHub Actions:
- Runs every Sunday at 00:00 UTC
- Can be manually triggered from Actions tab
- Updates `main` with upstream changes
- Rebases `main-custom` on updated `main`

### Manual Sync
Run the sync script to manually update from upstream:
```bash
./sync-upstream.sh
```

Or manually:
```bash
# Update main from upstream
git checkout main
git fetch upstream
git merge upstream/main
git push origin main

# Rebase custom branch
git checkout main-custom
git rebase main
git push origin main-custom --force-with-lease
```

### Handling Conflicts
If rebase conflicts occur:
1. Resolve conflicts keeping custom features
2. Continue rebase: `git rebase --continue`
3. Force push: `git push origin main-custom --force-with-lease`

## Contributing

### To Upstream
For features suitable for upstream:
1. Create branch from clean `main`
2. Submit PR to `Xuanwo/xlaude`

### To This Fork
For custom features:
1. Create branch from `main-custom`
2. Submit PR to `main-custom`

## Installation from Fork

```bash
cargo install --git https://github.com/laipz8200/xlaude --branch main-custom
```