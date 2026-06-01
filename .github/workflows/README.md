# GitHub Actions CI/CD Pipeline

This directory contains the automated CI/CD workflows for the Healthy-Stellar contracts repository.

## Workflows

### 1. CI Pipeline (`ci.yml`)

**Triggers:** Every push and pull request to `main` branch

**Jobs:**

#### Format Check
- Runs `cargo fmt --check` to enforce consistent code formatting
- Ensures all code follows Rust formatting standards
- **Required for merge**

#### Clippy Lint
- Runs `cargo clippy -- -D warnings` with zero warnings allowed
- Catches common mistakes and enforces best practices
- Checks all targets and features across the workspace
- **Required for merge**

#### Test Suite
- Runs `cargo test --workspace` to execute all 754+ tests
- Validates correctness across all contracts
- Uses caching to speed up subsequent runs
- **Required for merge**

#### Build WASM
- Builds all contracts for `wasm32-unknown-unknown` target
- Verifies that contracts compile to valid WASM
- Optimizes WASM binaries using `soroban contract optimize`
- Uploads WASM artifacts for 7 days
- **Required for merge**

#### CI Success
- Meta-job that depends on all other jobs
- Used for branch protection rules
- Fails if any CI job fails

**Caching Strategy:**
- Cargo registry cache
- Cargo git index cache
- Build target cache
- Significantly reduces CI run time

### 2. Testnet Deployment (`deploy-testnet.yml`)

**Triggers:**
- Automatic: Every push to `main` branch
- Manual: `workflow_dispatch` with optional contract selection

**Features:**

#### Change Detection
- Automatically detects which contracts changed in the push
- Only deploys contracts that have been modified
- Manual trigger allows deploying specific contracts

#### Build and Deploy
- Builds optimized WASM binaries
- Deploys to Stellar testnet using `soroban-cli`
- Uses GitHub Secrets for deployment credentials
- Generates deployment summary with contract IDs

#### Deployment Summary
- Creates markdown table with deployment results
- Shows success/failure status for each contract
- Includes deployed contract IDs
- Posts as PR comment (if applicable)
- Saves to GitHub Step Summary

#### Artifacts
- Saves deployment information for 90 days
- Stores contract IDs in `deployments/` directory
- Includes full deployment summary

**Required Secrets:**
- `STELLAR_SECRET_KEY` - Private key for testnet deployment account

### 3. Security Audit (`security-audit.yml`)

**Triggers:**
- Scheduled: Every Monday at 9:00 AM UTC
- Manual: `workflow_dispatch`
- Automatic: When `Cargo.toml` or `Cargo.lock` changes

**Jobs:**

#### Dependency Audit
- Runs `cargo audit` to check for known vulnerabilities
- Scans all dependencies in the workspace
- Generates JSON report with vulnerability details
- Creates GitHub Step Summary with results
- **Automatically creates/updates GitHub issues for vulnerabilities**

#### Dependency Review (PR only)
- Reviews dependency changes in pull requests
- Fails on moderate or higher severity issues
- Blocks licenses: GPL-3.0, AGPL-3.0
- Uses GitHub's dependency review API

#### Supply Chain Security
- Runs `cargo deny` for comprehensive checks:
  - Advisories: Known security vulnerabilities
  - Licenses: License compliance
  - Bans: Banned dependencies
  - Sources: Dependency source verification
- Generates Software Bill of Materials (SBOM)
- Uploads SBOM as artifact

**Artifacts:**
- Security audit results (90 days)
- SBOM (90 days)

**Issue Creation:**
- Automatically creates GitHub issues when vulnerabilities are found
- Labels: `security`, `dependencies`, `high-priority`
- Updates existing issues instead of creating duplicates
- Includes detailed vulnerability information and remediation steps

## Branch Protection Rules

To enforce CI requirements, configure the following branch protection rules for `main`:

1. **Require status checks to pass before merging**
   - Enable: "Require branches to be up to date before merging"
   - Required checks:
     - `CI Success`
     - `Format Check`
     - `Clippy Lint`
     - `Test Suite`
     - `Build WASM`

2. **Require pull request reviews before merging**
   - Require at least 1 approval

3. **Require conversation resolution before merging**

4. **Do not allow bypassing the above settings**

### Setting up Branch Protection

1. Go to repository Settings → Branches
2. Click "Add rule" or edit existing rule for `main`
3. Enable "Require status checks to pass before merging"
4. Search for and select the required checks listed above
5. Enable "Require branches to be up to date before merging"
6. Save changes

## Required Secrets

Configure the following secrets in repository Settings → Secrets and variables → Actions:

### Deployment Secrets

- **`STELLAR_SECRET_KEY`** (Required)
  - Private key for testnet deployment account
  - Format: `S...` (Stellar secret key)
  - Used by: `deploy-testnet.yml`
  - **Never commit this to the repository**

### Optional Secrets

- **`SLACK_WEBHOOK_URL`** (Optional)
  - Webhook URL for Slack notifications
  - Can be added for deployment notifications

## Local Testing

### Test CI Locally with Act

You can test GitHub Actions workflows locally using [act](https://github.com/nektos/act):

```bash
# Install act
# macOS: brew install act
# Linux: See https://github.com/nektos/act#installation

# Run CI workflow
act pull_request -W .github/workflows/ci.yml

# Run specific job
act -j test
```

### Manual CI Commands

Run the same checks locally before pushing:

```bash
# Format check
cargo fmt --all --check

# Clippy
cargo clippy --all-targets --all-features --workspace -- -D warnings

# Tests
cargo test --workspace

# WASM build
cargo build --release --target wasm32-unknown-unknown --workspace

# Security audit
cargo install cargo-audit
cargo audit
```

## Workflow Optimization

### Caching

All workflows use GitHub Actions caching to speed up runs:
- Cargo registry: `~/.cargo/registry`
- Cargo git index: `~/.cargo/git`
- Build artifacts: `target/`

Cache keys are based on `Cargo.lock` hash, ensuring cache invalidation when dependencies change.

### Parallel Execution

CI jobs run in parallel where possible:
- Format, Clippy, Test, and Build WASM run concurrently
- Only the final "CI Success" job waits for all others

### Conditional Execution

- Deployment only runs when contracts change
- Dependency review only runs on pull requests
- Issue creation only happens when vulnerabilities are found

## Monitoring and Alerts

### GitHub Notifications

- Failed CI runs trigger GitHub notifications
- Security issues create GitHub issues with labels
- Deployment summaries appear in PR comments

### Viewing Results

- **CI Status:** Check marks on commits and PRs
- **Deployment Info:** Check "Actions" tab → "Deploy to Testnet"
- **Security Audits:** Check "Actions" tab → "Security Audit"
- **Artifacts:** Download from workflow run pages

### Step Summaries

All workflows generate GitHub Step Summaries visible in the Actions UI:
- CI: Test results and build status
- Deployment: Contract IDs and deployment status
- Security: Vulnerability counts and SBOM preview

## Troubleshooting

### CI Failures

**Format Check Failed:**
```bash
cargo fmt --all
git add .
git commit -m "Fix formatting"
```

**Clippy Failed:**
```bash
cargo clippy --all-targets --all-features --workspace --fix
git add .
git commit -m "Fix clippy warnings"
```

**Tests Failed:**
```bash
cargo test --workspace
# Fix failing tests
```

**WASM Build Failed:**
```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --workspace
```

### Deployment Failures

**Secret Not Configured:**
- Ensure `STELLAR_SECRET_KEY` is set in repository secrets
- Verify the secret has the correct format

**Deployment Failed:**
- Check testnet RPC URL is accessible
- Verify account has sufficient XLM for deployment
- Check soroban-cli version compatibility

### Security Audit Failures

**Vulnerabilities Found:**
- Review the created GitHub issue
- Update affected dependencies to patched versions
- Run `cargo update` and test
- Commit updated `Cargo.lock`

**Cargo Audit Installation Failed:**
- Usually a transient issue, re-run the workflow
- Check cargo-audit compatibility with Rust version

## Maintenance

### Updating Workflows

When modifying workflows:
1. Test changes in a feature branch
2. Verify workflow syntax using GitHub's workflow editor
3. Test with a pull request before merging
4. Update this README if behavior changes

### Updating Dependencies

When updating Rust or soroban-cli versions:
1. Update in all workflow files consistently
2. Test locally first
3. Update caching keys if needed
4. Document version requirements

### Monitoring Costs

GitHub Actions provides:
- 2,000 minutes/month free for private repos
- Unlimited for public repos

Monitor usage in Settings → Billing → Actions

## Best Practices

1. **Always run CI locally before pushing**
2. **Keep workflows DRY** - Use reusable workflows for common tasks
3. **Use caching** - Speeds up CI significantly
4. **Fail fast** - Run quick checks (format, clippy) before slow ones (tests)
5. **Secure secrets** - Never log or expose secret values
6. **Monitor security** - Review security audit results weekly
7. **Keep dependencies updated** - Regularly update to patched versions

## Additional Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Soroban CLI Documentation](https://soroban.stellar.org/docs/tools/cli)
- [Cargo Audit Documentation](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
- [Stellar Testnet](https://soroban.stellar.org/docs/reference/testnet)
