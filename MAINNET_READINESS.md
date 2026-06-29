# Stellar Mainnet Go-Live Checklist

This document outlines the production readiness review required before deploying Healthy Stellar contracts to Stellar Mainnet.

## Pre-Deployment Requirements

### Security

- [ ] All contracts audited by an independent security firm
- [ ] All critical and high findings from the audit resolved and verified closed
- [ ] Penetration test of cross-contract call graph completed
- [ ] Admin keypairs stored in HSM or multi-sig wallet (not a single EOA)
- [ ] No hardcoded secrets or private keys in the codebase
- [ ] All authentication and access control mechanisms reviewed for bypass vulnerabilities

### Compliance

- [ ] HIPAA Security Rule gap analysis completed
- [ ] Data Processing Agreement with Stellar Foundation reviewed and signed
- [ ] GDPR data residency requirements assessed (acknowledge on-chain data is public and permanent)
- [ ] Legal review of storing PHI references on a public blockchain completed
- [ ] Documentation on data minimization and encryption strategies in place

### Operations

- [ ] TTL extension cron job configured and tested in production-like environment
- [ ] Incident response runbook published and tested
- [ ] On-call rotation established for contract emergency response
- [ ] Backup admin keypair stored in geographically separate cold storage
- [ ] Monitoring infrastructure deployed and tested
- [ ] Log aggregation and alerting configured
- [ ] Communication plan for deployment day finalized

### Contracts

- [ ] All 80 open issues resolved or explicitly deferred (with rationale documented)
- [ ] `cargo test --workspace` passes with zero compilation errors
- [ ] `cargo clippy --workspace` runs with no warnings
- [ ] WASM sizes verified within Stellar's contract size limit (current limit: 64 KB)
- [ ] `upgrade-governance` contract controls all production admin keys
- [ ] All contract interfaces reviewed and stabilized (API changes should be minimal post-launch)
- [ ] Deployment manifest published and verified (see SECURITY.md)
- [ ] Dry-run deployment executed against a Mainnet preview/staging environment

### Monitoring

- [ ] Alerting set up for TTL approaching threshold (< 24 hours)
- [ ] Dashboard for active prescriptions, trial enrolments, and claim counts deployed
- [ ] Anomaly detection for unusual transaction volume configured
- [ ] Metrics collection validated (response times, error rates, contract invocation counts)
- [ ] Baseline performance metrics recorded before launch

## Deployment Process

### Pre-Deployment Verification

1. **Code review**: All changes since previous release reviewed and tested
2. **Build verification**: Production WASM artifacts built and hashes recorded
3. **Manifest generation**: `./scripts/deploy_all.sh --network mainnet --dry-run` executed
4. **Smoke tests**: Simple invocations tested against a staging environment

### Deployment Execution

1. Configure `STELLAR_IDENTITY` to point to the production admin multi-sig account or HSM
2. Execute deployment with monitoring enabled:
   ```bash
   ./scripts/deploy_all.sh --network mainnet
   ```
3. Verify contract IDs in `deployments/mainnet.json` match the on-chain state
4. Record all deployed contract IDs in a secure, versioned log

### Post-Deployment Validation

- [ ] All contracts successfully initialized on Mainnet
- [ ] Governance contracts (`multisig-governance`, `upgrade-governance`) operational
- [ ] Each deployed contract responds to a no-op or read-only query
- [ ] Deployment manifest hashes verified against on-chain bytecode using Stellar Expert or Horizon API
- [ ] No unexpected errors or warnings in logs

## Sign-Off

Production readiness requires explicit sign-off from:

1. **Lead Developer** — confirms code quality, testing, and deployment plan
   - Name: ________________
   - Date: ________________
   - Signature: ________________

2. **Security Lead** — confirms audit findings resolved and security architecture sound
   - Name: ________________
   - Date: ________________
   - Signature: ________________

3. **Legal Counsel** — confirms compliance and data privacy requirements met
   - Name: ________________
   - Date: ________________
   - Signature: ________________

## Post-Launch Monitoring

After Mainnet launch:

- [ ] Monitor alerting dashboards for 72 hours continuously
- [ ] Weekly review of anomaly detection alerts for first month
- [ ] Monthly operational review with on-call team
- [ ] Quarterly security audit of governance decisions and contract state
- [ ] Incident postmortems completed within 24 hours of any production issue

## Rollback Plan

In case of critical issues post-launch:

1. **Minor issues**: Use `upgrade-governance` to deploy a patched version
2. **Critical issues**: Execute emergency governance proposal to pause high-risk functions
3. **Severe compromise**: Invoke emergency pause via multi-sig (if implemented)

Document any rollback decisions in the incident log and notify stakeholders.

## References

- [DEPLOYMENT.md](./DEPLOYMENT.md) — Deployment guide and procedures
- [SECURITY.md](./SECURITY.md) — Security architecture and policies
- [TTL_POLICY.md](./TTL_POLICY.md) — TTL management strategy
- Stellar Documentation: https://developers.stellar.org/
