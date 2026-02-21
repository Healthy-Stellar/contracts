use crate::types::{
    DataKey, EligibilityResult, Error, PrescriptionRequest, VirtualVisit, VisitStatus,
};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String, Symbol, Vec};

#[contract]
pub struct TelemedicineContract;

#[contractimpl]
impl TelemedicineContract {
    pub fn schedule_virtual_visit(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        visit_time: u64,
        visit_type: Symbol,
        duration_minutes: u32,
        platform: Symbol,
        consent_obtained: bool,
    ) -> Result<u64, Error> {
        patient_id.require_auth();

        let visit_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::VisitCount)
            .unwrap_or(0)
            + 1;

        let visit = VirtualVisit {
            visit_id,
            patient_id,
            provider_id: provider_id.clone(),
            scheduled_time: visit_time,
            visit_type,
            platform,
            status: VisitStatus::Scheduled,
            session_start: None,
            session_end: None,
            patient_location: String::from_str(&env, ""), // Default empty, updated at start
            consent_documented: consent_obtained,
        };

        env.storage()
            .persistent()
            .set(&DataKey::VirtualVisit(visit_id), &visit);
        env.storage()
            .instance()
            .set(&DataKey::VisitCount, &visit_id);

        env.events().publish(
            (Symbol::new(&env, "visit_scheduled"), visit_id),
            (provider_id, visit_time, duration_minutes),
        );

        Ok(visit_id)
    }

    pub fn start_virtual_session(
        env: Env,
        visit_id: u64,
        provider_id: Address,
        session_start_time: u64,
        patient_location_state: String,
    ) -> Result<String, Error> {
        provider_id.require_auth();

        let mut visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .ok_or(Error::VisitNotFound)?;

        if visit.provider_id != provider_id {
            return Err(Error::NotAuthorized);
        }

        if visit.status != VisitStatus::Scheduled {
            return Err(Error::InvalidStatusTransition);
        }

        // Let's assume validation happened via verify_telemedicine_eligibility before calling

        visit.status = VisitStatus::InProgress;
        visit.session_start = Some(session_start_time);
        visit.patient_location = patient_location_state;

        env.storage()
            .persistent()
            .set(&DataKey::VirtualVisit(visit_id), &visit);

        // Mock a simple session token
        let token = String::from_str(&env, "SESSION_TOKEN_123");
        env.events()
            .publish((Symbol::new(&env, "session_started"), visit_id), ());

        Ok(token)
    }

    pub fn record_visit_documentation(
        env: Env,
        visit_id: u64,
        provider_id: Address,
        visit_note_hash: BytesN<32>,
        diagnosis_codes: Vec<String>,
        assessment: String,
        plan: String,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .ok_or(Error::VisitNotFound)?;

        if visit.provider_id != provider_id {
            return Err(Error::NotAuthorized);
        }

        env.events().publish(
            (Symbol::new(&env, "visit_documented"), visit_id),
            (visit_note_hash, diagnosis_codes, assessment, plan),
        );

        Ok(())
    }

    pub fn end_virtual_session(
        env: Env,
        visit_id: u64,
        provider_id: Address,
        session_end_time: u64,
        session_duration: u32,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let mut visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .ok_or(Error::VisitNotFound)?;

        if visit.provider_id != provider_id {
            return Err(Error::NotAuthorized);
        }

        if visit.status != VisitStatus::InProgress {
            return Err(Error::InvalidStatusTransition);
        }

        visit.status = VisitStatus::Completed;
        visit.session_end = Some(session_end_time);

        env.storage()
            .persistent()
            .set(&DataKey::VirtualVisit(visit_id), &visit);
        env.events().publish(
            (Symbol::new(&env, "session_ended"), visit_id),
            session_duration,
        );

        Ok(())
    }

    pub fn verify_telemedicine_eligibility(
        env: Env,
        patient_id: Address,  // Unused in this mock, but present in signature
        provider_id: Address, // Unused in this mock, but present in signature
        patient_state: String,
        provider_state: String,
    ) -> Result<EligibilityResult, Error> {
        // Here we mock cross-state licensing validation.
        // If states are identical, they are eligible.
        // Otherwise, not eligible (in reality, would check a registry of allowed cross-state licenses).

        if patient_state == provider_state {
            Ok(EligibilityResult {
                is_eligible: true,
                reason: String::from_str(&env, "Same state"),
            })
        } else {
            Ok(EligibilityResult {
                is_eligible: false,
                reason: String::from_str(&env, "Cross-state practice not allowed in this mock"),
            })
        }
    }

    pub fn record_technical_issue(
        env: Env,
        visit_id: u64,
        reporter: Address,
        issue_type: Symbol,
        issue_description: String,
        resolution: Option<String>,
    ) -> Result<(), Error> {
        reporter.require_auth();

        let visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .ok_or(Error::VisitNotFound)?;

        if visit.provider_id != reporter && visit.patient_id != reporter {
            return Err(Error::NotAuthorized);
        }

        env.events().publish(
            (Symbol::new(&env, "technical_issue_recorded"), visit_id),
            (reporter, issue_type, issue_description, resolution),
        );

        Ok(())
    }

    pub fn prescribe_during_visit(
        env: Env,
        visit_id: u64,
        provider_id: Address,
        patient_id: Address,
        prescription_details: PrescriptionRequest,
    ) -> Result<u64, Error> {
        provider_id.require_auth();

        let visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .ok_or(Error::VisitNotFound)?;

        if visit.provider_id != provider_id {
            return Err(Error::NotAuthorized);
        }
        if visit.patient_id != patient_id {
            return Err(Error::NotAuthorized); // Mismatch between requested prescription patient and visit patient
        }

        // Mocking Rx ID generation
        let rx_id = env.ledger().timestamp() % 100000;

        env.events().publish(
            (Symbol::new(&env, "prescription_issued"), visit_id),
            (patient_id, prescription_details.medication_name, rx_id),
        );

        Ok(rx_id)
    }
}
