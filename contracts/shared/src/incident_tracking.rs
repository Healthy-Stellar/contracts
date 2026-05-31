#![no_std]

use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, String, Symbol, Vec};

/// Severity levels for incidents
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncidentSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Evidence type for structured diagnostics
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceType {
    ErrorLog,
    StateSnapshot,
    StackTrace,
    ContextData,
    ValidationFailure,
}

/// Individual piece of evidence
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evidence {
    pub evidence_type: EvidenceType,
    pub hash: Bytes, // Hash of evidence content
    pub recorded_at: u64,
    pub recorded_by: Address,
}

/// Structured incident record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Incident {
    pub incident_id: u64,
    pub severity: IncidentSeverity,
    pub contract: String,    // Which contract experienced the issue
    pub error_code: u32,     // Standardized error code
    pub description: String, // Short description
    pub reported_at: u64,
    pub reported_by: Address,
    pub evidence_count: u32,
    pub resolved: bool,
    pub resolution_note: Option<String>,
    pub correlation_id: Option<BytesN<32>>,
}

/// Evidence attachment to incident
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentEvidence {
    pub incident_id: u64,
    pub evidence_index: u32,
    pub evidence: Evidence,
    pub correlation_id: Option<BytesN<32>>,
}

/// Storage keys for incident tracking
#[contracttype]
pub enum IncidentKey {
    Admin,
    IncidentCounter,
    Incident(u64),
    IncidentEvidence(u64, u32),
    OpenIncidents,             // Vec<u64> - IDs of unresolved incidents
    ContractIncidents(String), // Contract-specific incident list
    CorrelationIndex(BytesN<32>), // correlation_id -> Vec<u64> incident IDs
}

/// Constants for incident tracking
pub const INCIDENT_CACHE_DURATION: u64 = 604800; // 7 days in seconds
pub const MAX_EVIDENCE_PER_INCIDENT: u32 = 100;

/// Create a structured incident report from error data
pub fn capture_incident(
    env: &Env,
    severity: IncidentSeverity,
    contract: String,
    error_code: u32,
    description: String,
    reporter: Address,
    correlation_id: Option<BytesN<32>>,
) -> u64 {
    let incident_id: u64 = env
        .storage()
        .instance()
        .get(&IncidentKey::IncidentCounter)
        .unwrap_or(0u64)
        + 1;
    env.storage()
        .instance()
        .set(&IncidentKey::IncidentCounter, &incident_id);

    let incident = Incident {
        incident_id,
        severity: severity.clone(),
        contract: contract.clone(),
        error_code,
        description,
        reported_at: env.ledger().timestamp(),
        reported_by: reporter,
        evidence_count: 0,
        resolved: false,
        resolution_note: None,
        correlation_id: correlation_id.clone(),
    };

    env.storage()
        .persistent()
        .set(&IncidentKey::Incident(incident_id), &incident);

    // Add to open incidents tracking
    let mut open: Vec<u64> = env
        .storage()
        .persistent()
        .get(&IncidentKey::OpenIncidents)
        .unwrap_or(Vec::new(env));
    open.push_back(incident_id);
    env.storage()
        .persistent()
        .set(&IncidentKey::OpenIncidents, &open);

    // Add to contract-specific tracking
    let mut contract_incidents: Vec<u64> = env
        .storage()
        .persistent()
        .get(&IncidentKey::ContractIncidents(contract.clone()))
        .unwrap_or(Vec::new(env));
    contract_incidents.push_back(incident_id);
    env.storage().persistent().set(
        &IncidentKey::ContractIncidents(contract),
        &contract_incidents,
    );

    // Index by correlation_id if provided
    if let Some(ref cid) = correlation_id {
        let mut correlated: Vec<u64> = env
            .storage()
            .persistent()
            .get(&IncidentKey::CorrelationIndex(cid.clone()))
            .unwrap_or(Vec::new(env));
        correlated.push_back(incident_id);
        env.storage().persistent().set(
            &IncidentKey::CorrelationIndex(cid.clone()),
            &correlated,
        );
    }

    // Emit event with correlation_id for off-chain aggregation
    env.events().publish(
        (Symbol::new(env, "incident"), Symbol::new(env, "captured")),
        (incident_id, correlation_id),
    );

    incident_id
}

/// Attach evidence to an incident
pub fn attach_evidence(
    env: &Env,
    incident_id: u64,
    evidence_type: EvidenceType,
    evidence_hash: Bytes,
    recorder: Address,
) -> Result<u32, ()> {
    let mut incident: Incident = env
        .storage()
        .persistent()
        .get(&IncidentKey::Incident(incident_id))
        .ok_or(())?;

    if incident.evidence_count >= MAX_EVIDENCE_PER_INCIDENT {
        return Err(());
    }

    let evidence_index = incident.evidence_count;
    let evidence = Evidence {
        evidence_type,
        hash: evidence_hash,
        recorded_at: env.ledger().timestamp(),
        recorded_by: recorder,
    };

    let incident_evidence = IncidentEvidence {
        incident_id,
        evidence_index,
        evidence,
        correlation_id: incident.correlation_id.clone(),
    };

    env.storage().persistent().set(
        &IncidentKey::IncidentEvidence(incident_id, evidence_index),
        &incident_evidence,
    );

    incident.evidence_count += 1;
    env.storage()
        .persistent()
        .set(&IncidentKey::Incident(incident_id), &incident);

    Ok(evidence_index)
}

/// Mark incident as resolved
pub fn resolve_incident(env: &Env, incident_id: u64, resolution_note: String) -> Result<(), ()> {
    let mut incident: Incident = env
        .storage()
        .persistent()
        .get(&IncidentKey::Incident(incident_id))
        .ok_or(())?;

    incident.resolved = true;
    incident.resolution_note = Some(resolution_note);

    env.storage()
        .persistent()
        .set(&IncidentKey::Incident(incident_id), &incident);

    // Remove from open incidents
    let mut open: Vec<u64> = env
        .storage()
        .persistent()
        .get(&IncidentKey::OpenIncidents)
        .unwrap_or(Vec::new(env));

    let mut new_open = Vec::new(env);
    for i in 0..open.len() {
        if let Some(id) = open.get(i) {
            if id != incident_id {
                new_open.push_back(id);
            }
        }
    }
    env.storage()
        .persistent()
        .set(&IncidentKey::OpenIncidents, &new_open);

    Ok(())
}

/// Get incident details
pub fn get_incident(env: &Env, incident_id: u64) -> Result<Incident, ()> {
    env.storage()
        .persistent()
        .get(&IncidentKey::Incident(incident_id))
        .ok_or(())
}

/// Get evidence for an incident
pub fn get_evidence(env: &Env, incident_id: u64, evidence_index: u32) -> Result<Evidence, ()> {
    let incident_evidence: IncidentEvidence = env
        .storage()
        .persistent()
        .get(&IncidentKey::IncidentEvidence(incident_id, evidence_index))
        .ok_or(())?;
    Ok(incident_evidence.evidence)
}

/// Get all incident IDs linked to a correlation ID
pub fn get_incidents_by_correlation_id(env: &Env, correlation_id: BytesN<32>) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&IncidentKey::CorrelationIndex(correlation_id))
        .unwrap_or(Vec::new(env))
}
