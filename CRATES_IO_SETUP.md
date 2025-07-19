# crates.io Publishing Setup

This document explains how to set up automatic publishing to crates.io for this project.

## Required Setup

### 1. Create a crates.io API Token

1. Visit [crates.io](https://crates.io/) and log in with your GitHub account
2. Go to [Account Settings > API Tokens](https://crates.io/settings/tokens)
3. Click "New Token"
4. Give it a descriptive name like "GitHub Actions - pv"
5. Select appropriate scopes (typically "publish-new" and "publish-update")
6. Copy the generated token immediately (you won't see it again)

### 2. Add Token to GitHub Repository Secrets

1. Go to your repository on GitHub
2. Navigate to Settings > Secrets and variables > Actions
3. Click "New repository secret"
4. Name: `CARGO_REGISTRY_TOKEN`
5. Value: Paste the token from step 1
6. Click "Add secret"

## Workflow Behavior

The release workflow now includes a `publish-crates` job that will:

1. ✅ Run only on version tags (e.g., `v0.2.0`)
2. ✅ Verify the tag matches the version in `Cargo.toml`
3. ✅ Publish the crate to crates.io
4. ✅ Only create the GitHub release if publishing succeeds

## Security Notes

- The API token is stored securely as a GitHub secret
- The token is only accessible to GitHub Actions in this repository
- If the token is compromised, revoke it immediately on crates.io
- The workflow verifies version consistency to prevent accidental publishes

## Manual Publishing

If needed, you can still publish manually:

```bash
cargo login <your-token>
cargo publish
```

## Troubleshooting

- **"crate already exists"**: You can't republish the same version
- **"invalid token"**: Check that the `CARGO_REGISTRY_TOKEN` secret is set correctly
- **"version mismatch"**: Ensure the git tag matches the version in `Cargo.toml`