# WASM Binary Size Baseline

This document establishes the baseline WASM binary sizes for all contracts before optimization work begins.

## Measurement Date

**Date:** To be measured on first CI run
**Commit:** To be recorded

## Size Limit

**Target:** 200 KB per contract (optimized)

## Baseline Sizes

To generate the baseline report, run:

```bash
make build-wasm
make measure-sizes
```

This will create:
- `wasm-size-report.md` - Detailed size report with all contracts
- `wasm-sizes.csv` - CSV data for analysis and tracking

## Expected Results

Based on typical Soroban contract sizes:
- **Small contracts** (registries, simple storage): 50-100 KB
- **Medium contracts** (business logic): 100-150 KB
- **Large contracts** (complex workflows): 150-200 KB

## Optimization Goals

### Phase 1: Immediate (Week 1)
- Measure all contract sizes
- Apply wasm-opt -O4 optimization
- Document baseline vs optimized sizes
- Identify contracts over 200 KB limit

### Phase 2: Short-term (Month 1)
- Extract shared code to libraries
- Remove unused dependencies
- Apply compiler optimizations
- Bring all contracts under 200 KB

### Phase 3: Long-term (Quarter 1)
- Continuous monitoring
- Prevent size regression
- Optimize new contracts
- Maintain <200 KB target

## Tracking

### Size Trends

Track size changes over time:
- Weekly measurements
- Compare against baseline
- Identify growing contracts
- Plan optimization work

### CI Integration

Automated size checks on every PR:
- Build and optimize all contracts
- Check against 200 KB limit
- Fail CI if limit exceeded
- Post size report as PR comment

## Contracts List

The repository contains approximately 42 contracts:

1. access-control
2. allergy-management
3. allergy-tracking
4. care-plan
5. clinical-guideline
6. clinical-trial
7. dental-records
8. doctor-registry
9. emergency-medical-info
10. financial-records
11. hai-tracking
12. health-records
13. healthcare-analytics
14. healthcare-credentialing
15. hospital-discharge-management
16. hospital-registry
17. imaging-radiology
18. immunization-registry
19. insurer-registry
20. lab-management
21. medical-claims
22. medical-device-tracking
23. mental-health
24. multisig-governance
25. nutrition-care-management
26. pacs-integration
27. patient-registry
28. patient-vitals
29. prenatal-pediatric
30. prescription-management
31. prior-authorization
32. provider-registry
33. referral
34. rehabilitation-services
35. shared (library)
36. telemedicine
37. ttl-config
38. upgrade-governance
39. zk-eligibility
40. zk-eligibility-verifier

Note: Some directories may be libraries or utilities, not deployable contracts.

## Optimization Strategies

### Compiler-Level
- `opt-level = "z"` for size optimization
- `lto = true` for link-time optimization
- `codegen-units = 1` for better optimization
- `strip = true` to remove debug symbols

### Code-Level
- Extract shared functionality
- Remove unused dependencies
- Optimize data structures
- Minimize macro usage

### Tool-Level
- wasm-opt -O4 for aggressive optimization
- wasm-strip to remove debug symbols
- soroban contract optimize as fallback

## Success Criteria

✅ All contracts measured and documented
✅ Optimization applied to all contracts
✅ All contracts under 200 KB limit
✅ CI checks enforce size limits
✅ Documentation complete

## Next Steps

1. Run initial measurement: `make measure-sizes`
2. Review baseline report
3. Identify contracts over limit
4. Apply optimization strategies
5. Re-measure and document results
6. Enable CI size checks

---

**Note:** This baseline will be updated with actual measurements once the optimization infrastructure is in place.
