# CI/CD Setup Guide

This guide walks you through setting up the complete CI/CD pipeline for the Healthy-Stellar contracts repository.

## Prerequisites

- Repository admin access
- Stellar testnet account with XLM for deployments
- Basic understanding of GitHub Actions

## Step 1: Configure Repository Secrets

### Required Secrets

1. Navigate to your repository on GitHub
2. Go to **Settings** → **Secrets and variables** → **Actions**
3. Click **New repository secret**
4. Add the following secret:

#### STELLAR_SECRET_KEY

- **Name:** `STELLAR_SECRET_KEY`
- **Value:** Your Stellar testnet secret key (starts with `S...`)
- **Purpose:** Used for deploying contracts to testnet

**How to get a testnet account:**

```bash
# Install soroban-cli
cargo install soroban-cli

# Generate a new identity
soroban keys generate deployer --network testnet

# Get the secret key
soroban keys show deployer

# Fund the account (get testnet XLM)
soroban keys address deployer
# Visit https://laboratory.stellar.org/#account-creator
# Paste the address and click "Create Account"
```

⚠️ **Security Warning:** Never commit secret keys to the repository or share them publicly.

## Step 2: Enable GitHub Actions

1. Go to **Settings** → **Actions** → **General**
2. Under "Actions permissions", select:
   - ✅ **Allow all actions and reusable workflows**
3. Under "Workflow permissions", select:
   - ✅ **Read and write permissions**
   - ✅ **Allow GitHub Actions to create and approve pull requests**
4. Click **Save**

## Step 3: Configure Branch Protection Rules

Protect the `main` branch to enforce CI requirements:

1. Go to **Settings** → **Branches**
2. Click **Add rule** (or edit existing rule for `main`)
3. Configure the following:

### Branch name pattern
```
main
```

### Protect matching branches

#### Require a pull request before merging
- ✅ Enable
- **Required approvals:** 1
- ✅ Dismiss stale pull request approvals when new commits are pushed
- ✅ Require review from Code Owners (if you have CODEOWNERS file)

#### Require status checks to pass before merging
- ✅ Enable
- ✅ Require branches to be up to date before merging
- **Required status checks:**
  - `CI Success`
  - `Format Check`
  - `Clippy Lint`
  - `Test Suite`
  - `Build WASM`

#### Require conversation resolution before merging
- ✅ Enable

#### Require signed commits
- ⬜ Optional (recommended for enhanced security)

#### Require linear history
- ✅ Enable (keeps git history clean)

#### Do not allow bypassing the above settings
- ✅ Enable (even for administrators)

4. Click **Create** or **Save changes**

## Step 4: Verify Workflows

### Test CI Workflow

1. Create a test branch:
```bash
git checkout -b test-ci
```

2. Make a small change (e.g., add a comment to a file)

3. Commit and push:
```bash
git add .
git commit -m "Test CI workflow"
git push origin test-ci
```

4. Create a pull request on GitHub

5. Verify that all CI checks run and pass:
   - Format Check ✅
   - Clippy Lint ✅
   - Test Suite ✅
   - Build WASM ✅
   - CI Success ✅

### Test Deployment Workflow

1. Merge a PR to `main` (or push directly if allowed)

2. Go to **Actions** tab → **Deploy to Testnet**

3. Verify the workflow runs and deploys contracts

4. Check the deployment summary in the workflow run

### Test Security Audit

1. Go to **Actions** tab → **Security Audit**

2. Click **Run workflow** → **Run workflow**

3. Verify the audit completes successfully

4. Check for any security issues in the summary

## Step 5: Configure Notifications (Optional)

### Email Notifications

GitHub automatically sends email notifications for:
- Failed workflow runs
- Security issues
- Pull request reviews

Configure in **Settings** → **Notifications**

### Slack Integration (Optional)

To receive Slack notifications:

1. Create a Slack webhook:
   - Go to your Slack workspace
   - Create an Incoming Webhook
   - Copy the webhook URL

2. Add as repository secret:
   - Name: `SLACK_WEBHOOK_URL`
   - Value: Your webhook URL

3. Add Slack notification step to workflows (example):

```yaml
- name: Notify Slack
  if: failure()
  uses: slackapi/slack-github-action@v1
  with:
    webhook-url: ${{ secrets.SLACK_WEBHOOK_URL }}
    payload: |
      {
        "text": "CI Failed for ${{ github.repository }}",
        "blocks": [
          {
            "type": "section",
            "text": {
              "type": "mrkdwn",
              "text": "❌ *CI Failed*\n*Repository:* ${{ github.repository }}\n*Branch:* ${{ github.ref }}\n*Commit:* ${{ github.sha }}"
            }
          }
        ]
      }
```

## Step 6: Set Up Dependabot (Optional but Recommended)

Automate dependency updates:

1. Create `.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10
    labels:
      - "dependencies"
      - "rust"
    commit-message:
      prefix: "chore"
      include: "scope"
```

2. Commit and push:
```bash
git add .github/dependabot.yml
git commit -m "Add Dependabot configuration"
git push
```

## Step 7: Monitor and Maintain

### Weekly Tasks

1. **Review Security Audit Results**
   - Check **Actions** → **Security Audit**
   - Review any created security issues
   - Update vulnerable dependencies

2. **Review Dependabot PRs**
   - Check for dependency update PRs
   - Review changelogs
   - Merge after CI passes

### Monthly Tasks

1. **Review Deployment History**
   - Check deployment artifacts
   - Verify contract IDs are documented
   - Clean up old artifacts if needed

2. **Update Workflows**
   - Check for GitHub Actions updates
   - Update Rust/soroban-cli versions if needed
   - Review and optimize caching strategies

### Quarterly Tasks

1. **Security Review**
   - Run comprehensive security audit
   - Review all dependencies
   - Update security policies

2. **Performance Review**
   - Analyze CI run times
   - Optimize slow jobs
   - Review caching effectiveness

## Troubleshooting

### CI Fails on First Run

**Problem:** CI fails with "required checks not found"

**Solution:** 
1. Let the CI run complete at least once
2. Then add the checks to branch protection
3. The checks must exist before they can be required

### Deployment Fails with "Secret not found"

**Problem:** `STELLAR_SECRET_KEY` not configured

**Solution:**
1. Verify secret is added in repository settings
2. Check secret name matches exactly (case-sensitive)
3. Ensure secret has correct format (starts with `S`)

### Security Audit Creates Too Many Issues

**Problem:** Multiple security issues created for same vulnerabilities

**Solution:**
1. The workflow checks for existing issues before creating new ones
2. If duplicates occur, manually close extras
3. Update the workflow to improve deduplication logic

### WASM Build Fails

**Problem:** Contracts don't compile to WASM

**Solution:**
1. Test locally: `cargo build --target wasm32-unknown-unknown`
2. Check for platform-specific dependencies
3. Ensure all contracts use `#![no_std]`
4. Review soroban-sdk compatibility

### Deployment Takes Too Long

**Problem:** Deployment workflow times out

**Solution:**
1. Deploy only changed contracts (automatic)
2. Use manual trigger to deploy specific contracts
3. Optimize WASM binaries before deployment
4. Consider parallel deployment (advanced)

## Advanced Configuration

### Custom Deployment Environments

Create separate workflows for different environments:

```yaml
# .github/workflows/deploy-mainnet.yml
name: Deploy to Mainnet

on:
  release:
    types: [published]

env:
  SOROBAN_NETWORK_PASSPHRASE: "Public Global Stellar Network ; September 2015"
  SOROBAN_RPC_URL: "https://soroban-mainnet.stellar.org"

# ... rest of deployment workflow
```

### Matrix Testing

Test across multiple Rust versions:

```yaml
test:
  strategy:
    matrix:
      rust: [stable, beta, nightly]
  steps:
    - uses: dtolnay/rust-toolchain@master
      with:
        toolchain: ${{ matrix.rust }}
    - run: cargo test --workspace
```

### Conditional Workflows

Run workflows only for specific paths:

```yaml
on:
  push:
    paths:
      - 'contracts/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
```

## Best Practices

1. **Test Locally First**
   - Run all CI checks locally before pushing
   - Use the commands in `.github/workflows/README.md`

2. **Keep Secrets Secure**
   - Never commit secrets to the repository
   - Rotate secrets regularly
   - Use separate accounts for testnet/mainnet

3. **Monitor CI Performance**
   - Review workflow run times
   - Optimize caching
   - Parallelize where possible

4. **Stay Updated**
   - Update GitHub Actions regularly
   - Keep Rust and soroban-cli current
   - Review security advisories weekly

5. **Document Changes**
   - Update README when workflows change
   - Document deployment procedures
   - Keep runbooks current

## Getting Help

- **GitHub Actions Issues:** [GitHub Community Forum](https://github.community/)
- **Soroban Issues:** [Soroban Discord](https://discord.gg/stellar)
- **Security Issues:** Create a private security advisory in the repository

## Checklist

Use this checklist to verify your setup:

- [ ] `STELLAR_SECRET_KEY` secret configured
- [ ] GitHub Actions enabled with write permissions
- [ ] Branch protection rules configured for `main`
- [ ] Required status checks added to branch protection
- [ ] CI workflow tested with a pull request
- [ ] Deployment workflow tested (manual trigger)
- [ ] Security audit workflow tested
- [ ] Notifications configured (email/Slack)
- [ ] Dependabot configured (optional)
- [ ] Team members have appropriate access levels
- [ ] Documentation reviewed and understood
- [ ] Monitoring and maintenance schedule established

## Next Steps

After completing this setup:

1. **Create a test PR** to verify all checks work
2. **Deploy a test contract** to verify deployment works
3. **Review security audit** results
4. **Train team members** on the CI/CD process
5. **Document any custom procedures** specific to your team

---

**Congratulations!** Your CI/CD pipeline is now set up and ready to ensure code quality and automate deployments for the Healthy-Stellar contracts.
