#407 Write contract upgrade guide covering schema migration and governance workflow
Repo Avatar
Healthy-Stellar/contracts
Summary
Upgrading a live Soroban contract requires multi-sig approval, schema compatibility checks, and coordination. There is no documentation walking operators through this process.

Guide Sections
Pre-upgrade checklist: schema compatibility, test coverage, staging deployment
Governance proposal submission: how to create and submit a WASM upgrade proposal
Multi-sig approval collection: minimum signers, timeout handling
Schema migration entry point: how to write and invoke migrate_schema
Rollback procedure: what to do if post-upgrade verification fails
Testnet dry run: how to simulate the upgrade before mainnet
Acceptance Criteria
 Guide written at docs/upgrade-guide.md
 Each section has a concrete example command or code snippet
 README links to the guide

 
40 matches
#408 Add performance benchmarks and gas cost documentation for contract operations
Repo Avatar
Healthy-Stellar/contracts
Summary
There are no documented baseline performance metrics for any contract operation. Developers and integrators have no reference for expected gas costs, latency, or resource consumption, making it difficult to optimize or budget for transactions.

Benchmarks to Document
Patient registration (single and batch)
Health record creation and retrieval (by type, by record ID)
Consent grant/revoke cycle
Prior authorization request → approval flow
Analytics report job (small, medium, large dataset)
Acceptance Criteria
 Benchmark harness integrated into the Makefile (make bench)
 Results documented in docs/benchmarks.md with methodology
 CI records benchmark results as artifacts for regression tracking
 Gas costs are expressed in both raw instructions and approximate XLM cost

 
40 matches
#409 Implement on-chain cost metering per contract operation
Repo Avatar
Healthy-Stellar/contracts
Summary
There is currently no per-operation cost tracking within contracts. In a healthcare system where multiple parties are billed for contract interactions (insurer, provider, patient), transparent on-chain cost accounting is important.

Proposed Design
Add a CostMeter struct to the shared module that tracks instruction counts per operation
Each contract entry point records its operation cost at start/end
Emit a OperationCost event with the operation name and resource consumption
Add a get_cumulative_cost(actor) query to retrieve total resource usage per actor
Acceptance Criteria
 CostMeter integrated into at least 3 high-traffic contracts as a pilot
 Costs are emitted as events with operation name and resource breakdown
 Cumulative cost query works correctly per actor
 Tests verify costs are recorded and not manipulable

#410 Add structured logging framework for contract debugging separate from audit events
Repo Avatar
Healthy-Stellar/contracts
Summary
Contract events serve as the immutable audit trail, but they are not well-suited for debugging. A structured logging layer that is active in test/dev environments (and stripped from production WASM) would accelerate debugging significantly.

Proposed Design
Add a debug_log!(env, level, message, context) macro to the shared module
In test builds: writes to a log buffer accessible via test assertions
In production builds: compiles to a no-op (zero overhead)
Log levels: DEBUG, INFO, WARN (mirroring standard conventions)
Acceptance Criteria
 Macro implemented and zero-cost in release builds (verified by WASM size delta)
 Test framework allows asserting log messages were emitted
 At least 5 contracts instrument their key code paths with debug logs
 Documentation explains the difference between logs (debug) and events (audit)
