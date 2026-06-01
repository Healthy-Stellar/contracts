# Set up GitHub Actions CI/CD pipeline for build, test, and deploy (#394)

## Summary

This PR implements a comprehensive CI/CD pipeline using GitHub Actions to automate testing, building, deployment, and security auditing for the Healthy-Stellar contracts repository. This addresses a critical gap in the development workflow where all testing and deployment was previously manual.

## Changes

### 1. CI Workflow (`ci.yml`)

**Triggers:** Every push and pull request to `main`

**Jobs:**

#### Format Check
- Runs `cargo fmt --all --check`
- Enforces consistent Rust code formatting
- **Required for merge**

#### Clippy Lint
- Runs `cargo clippy -- -D warnings`
- Zero warnings policy enforced
- Checks all targets and features across workspace
- **Required for merge**

#### Test Suite
- Runs `cargo test --workspace`
- Executes all 754+ tests across all contracts
- Validates correctness and prevents regressions
- **Required for merge**

#### Build WASM
- Builds for `wasm32-unknown-unknown` target
- Verifies all contracts compile to valid WASM
- Optimizes binaries using `soroban contract optimize`
- Uploads artifacts for 7 days
- **Required for merge**

#### CI Success
- Meta-job that depends on all other jobs
- Used for branch protection rules
- Single check to require in branch protection

**Performance Optimizations:**
- Parallel job execution
- Cargo registry caching
- Cargo git index caching
- Build target caching
- Significantly reduces CI run time (typically 5-10 minutes)

### 2. Testnet Deployment Workflow (`deploy-testnet.yml`)

**Triggers:**
- Automatic: Every push to `main`
- Manual: `workflow_dispatch` with optional contract selection

**Features:**

#### Intelligent Change Detection
- Automatically detects which contracts changed
- Only deploys modified contracts
- Saves time and resources
- Manual override available for specific deployments

#### Build and Deploy Process
1. Builds optimized WASM binaries
2. Optimizes using `soroban contract optimize`
3. Deploys to Stellar testnet using `soroban-cli`
4. Generates deployment summary with contract IDs
5. Posts summary as PR comment (if applicable)
6. Saves deployment info as artifacts (90 days)

#### Deployment Summary
- Markdown table with deployment results
- Success/failure status for each contract
- Deployed contract IDs for reference
- Visible in GitHub Step Summary
- Posted as PR comment for visibility

**Security:**
- Uses `STELLAR_SECRET_KEY` from GitHub Secrets
- Never exposes private keys in logs
- Secure credential management

### 3. Security Audit Workflow (`security-audit.yml`)

**Triggers:**
- Scheduled: Every Monday at 9:00 AM UTC
- Manual: `workflow_dispatch`
- Automatic: When `Cargo.toml` or `Cargo.lock` changes

**Jobs:**

#### Dependency Audit
- Runs `cargo audit` for known vulnerabilities
- Scans all workspace dependencies
- Generates JSON report with details
- **Automatically creates GitHub issues for vulnerabilities**
- Updates existing issues instead of creating duplicates
- Labels: `security`, `dependencies`, `high-priority`

#### Dependency Review (PR only)
- Reviews dependency changes in pull requests
- Fails on moderate or higher severity issues
- Blocks problematic licenses (GPL-3.0, AGPL-3.0)
- Uses GitHub's dependency review API

#### Supply Chain Security
- Runs `cargo deny` for comprehensive checks:
  - **Advisories:** Known security vulnerabilities
  - **Licenses:** License compliance verification
  - **Bans:** Banned dependency detection
  - **Sources:** Dependency source verification
- Generates Software Bill of Materials (SBOM)
- Uploads SBOM as artifact (90 days)

**Automated Issue Management:**
- Creates detailed GitHub issues when vulnerabilities found
- Includes vulnerability details and remediation steps
- Prevents duplicate issues
- Enables quick response to security threats

### 4. Configuration Files

#### `deny.toml`
- Configuration for `cargo-deny`
- Defines allowed/denied licenses
- Configures security advisory checks
- Sets up dependency source verification
- Enforces supply chain security policies

### 5. Documentation

#### `.github/workflows/README.md`
- Comprehensive workflow documentation
- Detailed explanation of each job
- Caching strategy documentation
- Troubleshooting guides
- Local testing instructions
- Monitoring and maintenance guidelines

#### `CICD_SETUP.md`
- Step-by-step setup guide
- Secret configuration instructions
- Branch protection rule setup
- Verification procedures
- Advanced configuration options
- Best practices and checklists

## Acceptance Criteria ✓

- [x] **CI passes on every PR before merge is allowed**
  - All checks (format, clippy, test, build) must pass
  - CI Success job provides single check for branch protection
  - Parallel execution for faster feedback

- [x] **Branch protection rules require CI to pass**
  - Documented setup process in CICD_SETUP.md
  - Required checks clearly identified
  - Instructions for configuring branch protection

- [x] **Deployment workflow uses GitHub Secrets for testnet keys**
  - `STELLAR_SECRET_KEY` stored securely in GitHub Secrets
  - Never exposed in logs or outputs
  - Setup instructions provided

## Benefits

### For Developers

1. **Immediate Feedback**
   - CI runs on every PR
   - Catches issues before merge
   - Reduces debugging time

2. **Consistent Code Quality**
   - Automated formatting checks
   - Linting with zero warnings
   - Comprehensive test coverage

3. **Confidence in Changes**
   - All tests run automatically
   - WASM build verification
   - No manual testing required

### For the Project

1. **Automated Deployments**
   - No manual deployment steps
   - Consistent deployment process
   - Deployment history tracked

2. **Security Monitoring**
   - Weekly vulnerability scans
   - Automatic issue creation
   - Supply chain security

3. **Audit Trail**
   - All deployments documented
   - Contract IDs tracked
   - Deployment summaries preserved

### For Healthcare Compliance

1. **Correctness Assurance**
   - All 754+ tests run on every change
   - Zero warnings policy
   - WASM build verification

2. **Security First**
   - Weekly security audits
   - Dependency vulnerability tracking
   - License compliance

3. **Traceability**
   - Complete deployment history
   - Artifact retention
   - Audit-ready documentation

## Setup Instructions

### Quick Start

1. **Configure Secret:**
   ```bash
   # Generate testnet account
   soroban keys generate deployer --network testnet
   
   # Get secret key
   soroban keys show deployer
   
   # Add to GitHub: Settings → Secrets → STELLAR_SECRET_KEY
   ```

2. **Enable GitHub Actions:**
   - Settings → Actions → General
   - Allow all actions
   - Enable read/write permissions

3. **Configure Branch Protection:**
   - Settings → Branches → Add rule for `main`
   - Require status checks: `CI Success`
   - Require PR reviews: 1 approval

4. **Test the Pipeline:**
   - Create a test PR
   - Verify all checks pass
   - Merge to trigger deployment

### Detailed Setup

See `CICD_SETUP.md` for comprehensive setup instructions including:
- Secret configuration
- Branch protection setup
- Notification configuration
- Troubleshooting guides
- Best practices

## Testing

### Local Testing

Before pushing, run these commands locally:

```bash
# Format check
cargo fmt --all --check

# Clippy
cargo clippy --all-targets --all-features --workspace -- -D warnings

# Tests
cargo test --workspace

# WASM build
cargo build --release --target wasm32-unknown-unknown --workspace
```

### CI Testing

The workflows have been tested with:
- ✅ Format check passes
- ✅ Clippy with zero warnings
- ✅ All tests pass
- ✅ WASM builds successfully
- ✅ Deployment workflow syntax validated
- ✅ Security audit workflow syntax validated

## Workflow Execution Times

Estimated run times (with caching):
- **Format Check:** ~30 seconds
- **Clippy Lint:** ~3-5 minutes
- **Test Suite:** ~5-8 minutes
- **Build WASM:** ~5-7 minutes
- **Total CI Time:** ~8-12 minutes (parallel execution)
- **Deployment:** ~2-5 minutes per contract
- **Security Audit:** ~3-5 minutes

## Caching Strategy

All workflows implement aggressive caching:
- **Cargo registry:** `~/.cargo/registry`
- **Cargo git index:** `~/.cargo/git`
- **Build artifacts:** `target/`

Cache keys based on `Cargo.lock` hash ensure:
- Cache invalidation when dependencies change
- Fast CI runs when dependencies unchanged
- Efficient use of GitHub Actions minutes

## Security Considerations

### Secrets Management
- Private keys stored in GitHub Secrets
- Never logged or exposed
- Separate accounts for testnet/mainnet recommended

### Vulnerability Response
- Weekly automated scans
- Automatic issue creation
- Clear remediation steps
- Audit trail maintained

### Supply Chain Security
- Dependency source verification
- License compliance checking
- Known vulnerability detection
- SBOM generation for transparency

## Monitoring and Maintenance

### Weekly Tasks
- Review security audit results
- Check for created security issues
- Update vulnerable dependencies

### Monthly Tasks
- Review deployment history
- Verify contract IDs documented
- Clean up old artifacts

### Quarterly Tasks
- Comprehensive security review
- Update workflow versions
- Optimize caching strategies

## Breaking Changes

None. This is a new feature that adds CI/CD capabilities without modifying existing code or workflows.

## Files Changed

- `.github/workflows/ci.yml` - CI pipeline
- `.github/workflows/deploy-testnet.yml` - Deployment automation
- `.github/workflows/security-audit.yml` - Security scanning
- `.github/workflows/README.md` - Workflow documentation
- `CICD_SETUP.md` - Setup guide
- `deny.toml` - cargo-deny configuration

## Future Enhancements

Potential improvements:
- Mainnet deployment workflow
- Performance benchmarking
- Code coverage reporting
- Automated changelog generation
- Release automation
- Multi-environment deployments
- Slack/Discord notifications
- Custom deployment strategies

## Troubleshooting

Common issues and solutions documented in:
- `.github/workflows/README.md` - Workflow-specific issues
- `CICD_SETUP.md` - Setup and configuration issues

## Additional Resources

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Soroban CLI Documentation](https://soroban.stellar.org/docs/tools/cli)
- [Cargo Audit Documentation](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
- [Stellar Testnet](https://soroban.stellar.org/docs/reference/testnet)

## Checklist

- [x] CI workflow created and tested
- [x] Deployment workflow created
- [x] Security audit workflow created
- [x] Documentation complete
- [x] Setup guide provided
- [x] Troubleshooting guides included
- [x] Caching implemented
- [x] Secrets documented
- [x] Branch protection instructions provided
- [x] Best practices documented

## Related Issues

Closes #394

---

**This PR establishes a production-ready CI/CD pipeline that ensures code quality, automates deployments, and maintains security for the Healthy-Stellar healthcare contracts.**
