use crate::types::{
    DataKey, EligibilityResult, Error, JurisdictionPolicy, ProviderLicense, PrescriptionRequest,
    ProviderSessionWindow, RateLimitConfig, SessionRecord, VirtualVisit, VisitStatus,
};
use soroban_sdk::{
    contract, contractclient, contractimpl, panic_with_error, xdr::ToXdr, Address, Bytes, BytesN,
    Env, String, Symbol, Vec,
};

const SESSION_TTL_SECONDS: u64 = 60 * 60;
const DEFAULT_MAX_SESSIONS_PER_WINDOW: u32 = 20;
const DEFAULT_WINDOW_DURATION_SECS: u64 = 86_400; // 24 hours

// ── Cross-contract interface for provider-registry verification ───────────────

#[contractclient(name = "ProviderRegistryClient")]
pub trait ProviderRegistryInterface {
    fn is_provider(env: Env, provider: Address) -> bool;
}

#[contract]
pub struct TelemedicineContract;

#[contractimpl]
impl TelemedicineContract {
    /// Get the current rate limit configuration, or return defaults if not set.
    fn get_rate_limit_config(env: &Env) -> RateLimitConfig {
        env.storage()
            .persistent()
            .get::<_, RateLimitConfig>(&DataKey::RateLimitConfig)
            .unwrap_or(RateLimitConfig {
                max_sessions_per_window: DEFAULT_MAX_SESSIONS_PER_WINDOW,
                window_duration_secs: DEFAULT_WINDOW_DURATION_SECS,
            })
    }

    /// Check if provider has exceeded their session creation rate limit.
    /// If not exceeded, increments counter and returns Ok.
    /// If window has expired, resets the counter.
    fn check_and_update_rate_limit(
        env: &Env,
        provider_id: &Address,
    ) -> Result<(), Error> {
        let config = Self::get_rate_limit_config(env);
        let now = env.ledger().timestamp();
        let window_key = DataKey::ProviderSessionWindow(provider_id.clone());

        let mut window: ProviderSessionWindow = env
            .storage()
            .persistent()
            .get(&window_key)
            .unwrap_or(ProviderSessionWindow {
                session_count: 0,
                window_start: now,
            });

        // Check if window has expired and reset if needed
        if now >= window.window_start + config.window_duration_secs {
            window.session_count = 0;
            window.window_start = now;
        }

        // Check if limit would be exceeded
        if window.session_count >= config.max_sessions_per_window {
            return Err(Error::RateLimitExceeded);
        }

        // Increment counter and save
        window.session_count += 1;
        env.storage().persistent().set(&window_key, &window);

        Ok(())
    }

    /// Set the rate limit configuration. Callable by any admin.
    pub fn set_rate_limit_config(
        env: Env,
        admin: Address,
        max_sessions_per_window: u32,
        window_duration_secs: u64,
    ) -> Result<(), Error> {
        admin.require_auth();

        let config = RateLimitConfig {
            max_sessions_per_window,
            window_duration_secs,
        };

        env.storage()
            .persistent()
            .set(&DataKey::RateLimitConfig, &config);
        Ok(())
    }

    pub fn schedule_virtual_visit(
        env: Env,
        patient_id: Address,
        provider_id: Address,
        visit_time: u64,
        visit_type: Symbol,
        duration_minutes: u32,
        platform: Symbol,
        consent_obtained: bool,
        recording_consent: bool,
    ) -> Result<u64, Error> {
        patient_id.require_auth();

        // Verify provider is actively registered in the provider-registry.
        if let Some(registry_addr) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::ProviderRegistryAddress)
        {
            let registry = ProviderRegistryClient::new(&env, &registry_addr);
            if !registry.is_provider(&provider_id) {
                return Err(Error::ProviderNotRegistered);
            }
        }

        let visit_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::VisitCount)
            .unwrap_or(0)
            + 1;

        let visit = VirtualVisit {
            visit_id,
            patient_id: patient_id.clone(),
            provider_id: provider_id.clone(),
            scheduled_time: visit_time,
            visit_type,
            platform,
            status: VisitStatus::Scheduled,
            session_start: None,
            session_end: None,
            patient_location: String::from_str(&env, ""),
            consent_documented: consent_obtained,
            recording_consent: Some(recording_consent),
        };

        env.storage()
            .persistent()
            .set(&DataKey::VirtualVisit(visit_id), &visit);
        env.storage()
            .instance()
            .set(&DataKey::VisitCount, &visit_id);

        // Emit consent event per HIPAA requirement.
        if recording_consent {
            env.events().publish(
                (Symbol::new(&env, "recording_consent_granted"), visit_id),
                patient_id.clone(),
            );
        } else {
            env.events().publish(
                (Symbol::new(&env, "recording_consent_denied"), visit_id),
                patient_id.clone(),
            );
        }

        env.events().publish(
            (Symbol::new(&env, "visit_scheduled"), visit_id),
            (provider_id, visit_time, duration_minutes),
        );

        Ok(visit_id)
    }

    /// Store recording metadata. Requires recording_consent = true on the session.
    pub fn store_recording_metadata(
        env: Env,
        visit_id: u64,
        provider_id: Address,
        recording_hash: BytesN<32>,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .ok_or_else(|| {
                panic_with_error!(&env, Error::VisitNotFound);
            })?;

        if visit.provider_id != provider_id {
            panic_with_error!(&env, Error::NotAuthorized);
        }

        match visit.recording_consent {
            Some(true) => {}
            _ => panic_with_error!(&env, Error::RecordingConsentRequired),
        }

        env.events().publish(
            (Symbol::new(&env, "recording_stored"), visit_id),
            recording_hash,
        );

        Ok(())
    }

    pub fn start_virtual_session(
        env: Env,
        visit_id: u64,
        provider_id: Address,
        session_start_time: u64,
        patient_location_state: String,
        provider_state: String,
    ) -> Result<BytesN<32>, Error> {
        provider_id.require_auth();

        // Check rate limit: max sessions per provider per window
        Self::check_and_update_rate_limit(&env, &provider_id)?;

        let mut visit: VirtualVisit = env
            .storage()
            .persistent()
            .get(&DataKey::VirtualVisit(visit_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::VisitNotFound));

        if visit.provider_id != provider_id {
            panic_with_error!(&env, Error::NotAuthorized);
        }

        if visit.status != VisitStatus::Scheduled {
            panic_with_error!(&env, Error::InvalidStatusTransition);
        }

        // Enforce eligibility before allowing session start.
        let eligibility = Self::verify_telemedicine_eligibility(
            env.clone(),
            visit.patient_id.clone(),
            provider_id.clone(),
            patient_location_state.clone(),
            provider_state,
        )?;
        if !eligibility.is_eligible {
            return Err(Error::IneligibleLocation);
        }

        visit.status = VisitStatus::InProgress;
        visit.session_start = Some(session_start_time);
        visit.patient_location = patient_location_state;

        env.storage()
            .persistent()
            .set(&DataKey::VirtualVisit(visit_id), &visit);

        let nonce = env
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::SessionNonce)
            .unwrap_or(0)
            + 1;
        env.storage().instance().set(&DataKey::SessionNonce, &nonce);

        let token = compute_session_token(
            &env,
            visit_id,
            &provider_id,
            &visit.patient_id,
            session_start_time,
            nonce,
        );
        let token_hash = hash_token(&env, &token);
        let session = SessionRecord {
            token_hash,
            visit_id,
            caller: provider_id.clone(),
            expires_at: session_start_time + SESSION_TTL_SECONDS,
            used: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Session(visit_id), &session);

        env.events()
            .publish((Symbol::new(&env, "session_started"), visit_id), ());

        Ok(token)
    }

    pub fn validate_session_token(
        env: Env,
        visit_id: u64,
        caller: Address,
        token: BytesN<32>,
    ) -> Result<(), Error> {
        caller.require_auth();

        let mut session: SessionRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Session(visit_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::InvalidSessionToken));

        if session.visit_id != visit_id || session.caller != caller {
            panic_with_error!(&env, Error::InvalidSessionToken);
        }
        if session.used {
            panic_with_error!(&env, Error::SessionAlreadyUsed);
        }
        if env.ledger().timestamp() > session.expires_at {
            panic_with_error!(&env, Error::SessionExpired);
        }
        if session.token_hash != hash_token(&env, &token) {
            panic_with_error!(&env, Error::InvalidSessionToken);
        }

        session.used = true;
        env.storage()
            .persistent()
            .set(&DataKey::Session(visit_id), &session);

        Ok(())
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
            .unwrap_or_else(|| panic_with_error!(&env, Error::VisitNotFound));

        if visit.provider_id != provider_id {
            panic_with_error!(&env, Error::NotAuthorized);
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
            .unwrap_or_else(|| panic_with_error!(&env, Error::VisitNotFound));

        if visit.provider_id != provider_id {
            panic_with_error!(&env, Error::NotAuthorized);
        }

        if visit.status != VisitStatus::InProgress {
            panic_with_error!(&env, Error::InvalidStatusTransition);
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
        _patient_id: Address,
        provider_id: Address,
        patient_state: String,
        provider_state: String,
    ) -> Result<EligibilityResult, Error> {
        let now = env.ledger().timestamp();

        // 1. Look up the provider's license for the patient's jurisdiction (where care is delivered).
        let license_key = DataKey::LicenseRegistry(provider_id.clone(), patient_state.clone());
        let home_license: Option<ProviderLicense> =
            env.storage().persistent().get(&license_key);

        // 2. Check direct license in patient's state.
        if let Some(ref lic) = home_license {
            if lic.active && (lic.valid_until == 0 || lic.valid_until > now) {
                return Ok(EligibilityResult {
                    is_eligible: true,
                    reason: String::from_str(&env, "Licensed in patient jurisdiction"),
                });
            }
            if lic.active && lic.valid_until > 0 && lic.valid_until <= now {
                return Err(Error::LicenseExpired);
            }
        }

        // 3. Same-state shortcut: if provider is licensed in their own state and
        //    patient_state == provider_state, allow (no cross-state issue).
        if patient_state == provider_state {
            let provider_home_key =
                DataKey::LicenseRegistry(provider_id.clone(), provider_state.clone());
            let provider_home: Option<ProviderLicense> =
                env.storage().persistent().get(&provider_home_key);
            if let Some(lic) = provider_home {
                if lic.active && (lic.valid_until == 0 || lic.valid_until > now) {
                    return Ok(EligibilityResult {
                        is_eligible: true,
                        reason: String::from_str(&env, "Same jurisdiction"),
                    });
                }
            }
        }

        // 4. Cross-state: check jurisdiction policy for compact membership.
        let policy_key = DataKey::JurisdictionPolicy(patient_state.clone());
        let policy: Option<JurisdictionPolicy> = env.storage().persistent().get(&policy_key);

        if let Some(pol) = policy {
            if pol.allows_compact {
                // Check if provider holds a valid license in any compact-member state.
                // compact_members is a comma-separated list stored as a String.
                // We iterate by scanning for provider_state within the list.
                if string_contains_state(&env, &pol.compact_members, &provider_state) {
                    // Verify provider has a valid license in their home state.
                    let provider_home_key =
                        DataKey::LicenseRegistry(provider_id.clone(), provider_state.clone());
                    let provider_home: Option<ProviderLicense> =
                        env.storage().persistent().get(&provider_home_key);
                    if let Some(lic) = provider_home {
                        if lic.active && (lic.valid_until == 0 || lic.valid_until > now) {
                            return Ok(EligibilityResult {
                                is_eligible: true,
                                reason: String::from_str(&env, "Compact interstate license"),
                            });
                        }
                    }
                }
            }
        }

        Ok(EligibilityResult {
            is_eligible: false,
            reason: String::from_str(&env, "No valid license for patient jurisdiction"),
        })
    }

    /// Register or update a provider's license for a jurisdiction.
    /// Only the provider themselves may register their own license.
    pub fn register_provider_license(
        env: Env,
        provider_id: Address,
        jurisdiction: String,
        license_number: String,
        valid_until: u64,
    ) -> Result<(), Error> {
        provider_id.require_auth();

        let license = ProviderLicense {
            provider_id: provider_id.clone(),
            jurisdiction: jurisdiction.clone(),
            license_number,
            valid_until,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::LicenseRegistry(provider_id, jurisdiction), &license);
        Ok(())
    }

    /// Set jurisdiction policy (compact membership). Callable by any authorized admin.
    pub fn set_jurisdiction_policy(
        env: Env,
        admin: Address,
        jurisdiction: String,
        allows_compact: bool,
        compact_members: String,
    ) -> Result<(), Error> {
        admin.require_auth();

        let policy = JurisdictionPolicy {
            jurisdiction: jurisdiction.clone(),
            allows_compact,
            compact_members,
        };

        env.storage()
            .persistent()
            .set(&DataKey::JurisdictionPolicy(jurisdiction), &policy);
        Ok(())
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
            .unwrap_or_else(|| panic_with_error!(&env, Error::VisitNotFound));

        if visit.provider_id != reporter && visit.patient_id != reporter {
            panic_with_error!(&env, Error::NotAuthorized);
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
            .unwrap_or_else(|| panic_with_error!(&env, Error::VisitNotFound));

        if visit.provider_id != provider_id {
            panic_with_error!(&env, Error::NotAuthorized);
        }
        if visit.patient_id != patient_id {
            panic_with_error!(&env, Error::NotAuthorized);
        }

        // Prescribing is only allowed during an active session (#563).
        if visit.status != VisitStatus::InProgress {
            panic_with_error!(&env, Error::SessionNotActive);
        }

        // Confirm provider holds a valid license in the patient's jurisdiction (#563).
        let patient_state = &visit.patient_location;
        if patient_state.len() > 0 {
            let now = env.ledger().timestamp();
            let lic_key =
                DataKey::LicenseRegistry(provider_id.clone(), patient_state.clone());
            let has_license = env
                .storage()
                .persistent()
                .get::<_, ProviderLicense>(&lic_key)
                .map(|l| l.active && (l.valid_until == 0 || l.valid_until > now))
                .unwrap_or(false);
            if !has_license {
                panic_with_error!(&env, Error::ProviderNotLicensedInPatientState);
            }

            // Stricter enforcement for controlled substances (#563).
            if prescription_details.is_controlled_substance {
                let requires_inperson: bool = env
                    .storage()
                    .persistent()
                    .get(&DataKey::ControlledSubstancePolicy(patient_state.clone()))
                    .unwrap_or(false);
                if requires_inperson {
                    panic_with_error!(&env, Error::ControlledSubstanceRequiresInPerson);
                }
            }
        }

        let rx_id = env.ledger().timestamp() % 100000;

        env.events().publish(
            (Symbol::new(&env, "prescription_issued"), visit_id),
            (patient_id, prescription_details.medication_name, rx_id),
        );

        Ok(rx_id)
    }

    /// Set controlled-substance prescribing policy for a jurisdiction (#563).
    ///
    /// When `requires_inperson` is true, providers cannot prescribe controlled
    /// substances (DEA schedule I–V) during a telemedicine session in that jurisdiction.
    pub fn set_controlled_substance_policy(
        env: Env,
        admin: Address,
        jurisdiction: String,
        requires_inperson: bool,
    ) -> Result<(), Error> {
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::ControlledSubstancePolicy(jurisdiction), &requires_inperson);
        Ok(())
    }
}

fn compute_session_token(
    env: &Env,
    visit_id: u64,
    provider_id: &Address,
    patient_id: &Address,
    session_start_time: u64,
    nonce: u64,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_array(&visit_id.to_be_bytes());
    data.append(&provider_id.clone().to_xdr(env));
    data.append(&patient_id.clone().to_xdr(env));
    data.extend_from_array(&session_start_time.to_be_bytes());
    data.extend_from_array(&nonce.to_be_bytes());
    env.crypto().sha256(&data).into()
}

fn hash_token(env: &Env, token: &BytesN<32>) -> BytesN<32> {
    env.crypto().sha256(&token.clone().to_xdr(env)).into()
}

/// Check whether `state` appears as a comma-separated token inside `list`.
/// Both are Soroban `String`s; we compare byte-by-byte via XDR encoding.
fn string_contains_state(env: &Env, list: &String, state: &String) -> bool {
    let list_bytes = list.clone().to_xdr(env);
    let state_bytes = state.clone().to_xdr(env);

    // XDR String: 4-byte big-endian content length, then content bytes, then 0-3 padding.
    if list_bytes.len() < 4 || state_bytes.len() < 4 {
        return false;
    }

    // Read content lengths from XDR prefix.
    let lc = ((list_bytes.get(0).unwrap_or(0) as u32) << 24
        | (list_bytes.get(1).unwrap_or(0) as u32) << 16
        | (list_bytes.get(2).unwrap_or(0) as u32) << 8
        | (list_bytes.get(3).unwrap_or(0) as u32)) as usize;

    let sc = ((state_bytes.get(0).unwrap_or(0) as u32) << 24
        | (state_bytes.get(1).unwrap_or(0) as u32) << 16
        | (state_bytes.get(2).unwrap_or(0) as u32) << 8
        | (state_bytes.get(3).unwrap_or(0) as u32)) as usize;

    if sc == 0 || sc > lc {
        return false;
    }

    // Slide a window of size `sc` over the list content (offset 4 in XDR bytes).
    let mut i: usize = 0;
    while i + sc <= lc {
        let mut matched = true;
        for j in 0..sc {
            let lb = list_bytes.get((4 + i + j) as u32).unwrap_or(0);
            let sb = state_bytes.get((4 + j) as u32).unwrap_or(1);
            if lb != sb {
                matched = false;
                break;
            }
        }
        if matched {
            let before_ok = i == 0 || list_bytes.get((4 + i - 1) as u32).unwrap_or(0) == b',';
            let after_ok = (i + sc) == lc
                || list_bytes.get((4 + i + sc) as u32).unwrap_or(0) == b',';
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}
