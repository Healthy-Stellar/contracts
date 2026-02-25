#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    InvalidData = 2,
    InvalidInfectionType = 3,
    InvalidSusceptibility = 4,
    InvalidPrecautionType = 5,
    InvalidPriority = 6,
    DivisionByZero = 7,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AntibioticSusceptibility {
    pub antibiotic: String,
    pub susceptibility: Symbol,
    pub mic_value_x100: Option<i64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Organism {
    pub name: String,
    pub specimen_type: Symbol,
    pub collection_date: u64,
    pub culture_result_hash: BytesN<32>,
    pub susceptibilities: Vec<AntibioticSusceptibility>,
    pub is_multidrug_resistant: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfectionCase {
    pub infection_id: u64,
    pub patient_id: Address,
    pub facility_id: Address,
    pub infection_type: Symbol,
    pub onset_date: u64,
    pub location: String,
    pub organisms: Vec<Organism>,
    pub device_associated: bool,
    pub device_days: Option<u32>,
    pub reported_by: Address,
    pub outbreak_related: bool,
    pub resolved: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfectionRate {
    pub infection_type: Symbol,
    pub numerator: u32,
    pub denominator: u32,
    pub rate_per_1000_days_x100: i64,
    pub sir_x100: Option<i64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutbreakCluster {
    pub outbreak_id: u64,
    pub infection_type: Symbol,
    pub facility_id: Address,
    pub unit: String,
    pub case_count: u32,
    pub start_date: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationPrecaution {
    pub precaution_id: u64,
    pub patient_id: Address,
    pub precaution_type: Symbol,
    pub start_date: u64,
    pub indication: String,
    pub discontinuation_criteria: String,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandHygieneRecord {
    pub record_id: u64,
    pub facility_id: Address,
    pub unit: String,
    pub observation_date: u64,
    pub opportunities: u32,
    pub compliant_actions: u32,
    pub compliance_rate_x100: i64,
    pub observer: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AntibioticStewardshipRecord {
    pub record_id: u64,
    pub facility_id: Address,
    pub antibiotic: String,
    pub days_of_therapy: u32,
    pub patient_days: u32,
    pub reporting_period: u64,
    pub dot_per_1000_days_x100: i64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NhsnReport {
    pub report_id: u64,
    pub facility_id: Address,
    pub reporting_month: u64,
    pub infection_data_hash: BytesN<32>,
    pub device_utilization_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    InfectionCase(u64),
    OutbreakCluster(u64),
    IsolationPrecaution(u64),
    HandHygieneRecord(u64),
    StewardshipRecord(u64),
    NhsnReport(u64),
    InfectionIds,
    OutbreakIds,
    PrecautionIds,
}

#[contract]
pub struct HAITrackingContract;

#[contractimpl]
impl HAITrackingContract {
    pub fn report_infection(
        env: Env,
        patient_id: Address,
        facility_id: Address,
        infection_type: Symbol,
        onset_date: u64,
        location: String,
        device_associated: bool,
        device_days: Option<u32>,
        reported_by: Address,
    ) -> Result<u64, Error> {
        reported_by.require_auth();

        if !Self::is_valid_infection_type(&env, &infection_type) {
            return Err(Error::InvalidInfectionType);
        }

        if device_associated && device_days.is_none() {
            return Err(Error::InvalidData);
        }

        let infection_id = Self::next_id(&env, symbol_short!("inf_ctr"));
        let case = InfectionCase {
            infection_id,
            patient_id,
            facility_id,
            infection_type,
            onset_date,
            location,
            organisms: Vec::new(&env),
            device_associated,
            device_days,
            reported_by,
            outbreak_related: false,
            resolved: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::InfectionCase(infection_id), &case);
        Self::push_id(&env, DataKey::InfectionIds, infection_id);

        Ok(infection_id)
    }

    pub fn record_organism(
        env: Env,
        infection_id: u64,
        organism_name: String,
        specimen_type: Symbol,
        collection_date: u64,
        culture_result_hash: BytesN<32>,
    ) -> Result<(), Error> {
        let mut case = Self::get_infection_case_internal(&env, infection_id)?;

        let organism = Organism {
            name: organism_name,
            specimen_type,
            collection_date,
            culture_result_hash,
            susceptibilities: Vec::new(&env),
            is_multidrug_resistant: false,
        };

        case.organisms.push_back(organism);
        env.storage()
            .persistent()
            .set(&DataKey::InfectionCase(infection_id), &case);

        Ok(())
    }

    pub fn record_antibiotic_susceptibility(
        env: Env,
        infection_id: u64,
        organism_name: String,
        antibiotic: String,
        susceptibility: Symbol,
        mic_value_x100: Option<i64>,
    ) -> Result<(), Error> {
        if !Self::is_valid_susceptibility(&env, &susceptibility) {
            return Err(Error::InvalidSusceptibility);
        }

        let mut case = Self::get_infection_case_internal(&env, infection_id)?;
        let mut idx = 0u32;
        let mut found = false;
        while idx < case.organisms.len() {
            if let Some(mut org) = case.organisms.get(idx) {
                if org.name == organism_name {
                    org.susceptibilities.push_back(AntibioticSusceptibility {
                        antibiotic,
                        susceptibility,
                        mic_value_x100,
                    });

                    let mut resistant_count = 0u32;
                    let mut s_idx = 0u32;
                    while s_idx < org.susceptibilities.len() {
                        if let Some(sus) = org.susceptibilities.get(s_idx) {
                            if sus.susceptibility == Symbol::new(&env, "resistant") {
                                resistant_count += 1;
                            }
                        }
                        s_idx += 1;
                    }
                    org.is_multidrug_resistant = resistant_count >= 3;
                    case.organisms.set(idx, org);
                    found = true;
                    break;
                }
            }
            idx += 1;
        }

        if !found {
            return Err(Error::NotFound);
        }

        env.storage()
            .persistent()
            .set(&DataKey::InfectionCase(infection_id), &case);

        Ok(())
    }

    pub fn identify_outbreak_cluster(
        env: Env,
        infection_type: Symbol,
        facility_id: Address,
        unit: String,
        time_window_days: u32,
        case_threshold: u32,
    ) -> Result<Option<u64>, Error> {
        if !Self::is_valid_infection_type(&env, &infection_type) || case_threshold == 0 {
            return Err(Error::InvalidData);
        }

        let now = env.ledger().timestamp();
        let window_seconds = u64::from(time_window_days) * 86_400;
        let window_start = now.saturating_sub(window_seconds);

        let infection_ids = Self::get_ids(&env, DataKey::InfectionIds);
        let mut case_count = 0u32;

        let mut i = 0u32;
        while i < infection_ids.len() {
            if let Some(case_id) = infection_ids.get(i) {
                if let Ok(mut case) = Self::get_infection_case_internal(&env, case_id) {
                    if case.infection_type == infection_type
                        && case.facility_id == facility_id
                        && case.location == unit
                        && case.onset_date >= window_start
                    {
                        case_count += 1;
                        case.outbreak_related = true;
                        env.storage()
                            .persistent()
                            .set(&DataKey::InfectionCase(case_id), &case);
                    }
                }
            }
            i += 1;
        }

        if case_count < case_threshold {
            return Ok(None);
        }

        let outbreak_id = Self::next_id(&env, symbol_short!("out_ctr"));
        let cluster = OutbreakCluster {
            outbreak_id,
            infection_type,
            facility_id,
            unit,
            case_count,
            start_date: window_start,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::OutbreakCluster(outbreak_id), &cluster);
        Self::push_id(&env, DataKey::OutbreakIds, outbreak_id);

        Ok(Some(outbreak_id))
    }

    pub fn initiate_outbreak_investigation(
        env: Env,
        outbreak_id: u64,
        lead_investigator: Address,
        investigation_protocol: String,
    ) -> Result<(), Error> {
        lead_investigator.require_auth();
        let outbreak: OutbreakCluster = env
            .storage()
            .persistent()
            .get(&DataKey::OutbreakCluster(outbreak_id))
            .ok_or(Error::NotFound)?;

        env.events().publish(
            (Symbol::new(&env, "outbreak_investigation"), outbreak_id),
            (lead_investigator, investigation_protocol, outbreak.unit),
        );

        Ok(())
    }

    pub fn track_isolation_precaution(
        env: Env,
        patient_id: Address,
        precaution_type: Symbol,
        start_date: u64,
        indication: String,
        discontinuation_criteria: String,
    ) -> Result<u64, Error> {
        if !Self::is_valid_precaution(&env, &precaution_type) {
            return Err(Error::InvalidPrecautionType);
        }

        let precaution_id = Self::next_id(&env, symbol_short!("iso_ctr"));
        let record = IsolationPrecaution {
            precaution_id,
            patient_id,
            precaution_type,
            start_date,
            indication,
            discontinuation_criteria,
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::IsolationPrecaution(precaution_id), &record);
        Self::push_id(&env, DataKey::PrecautionIds, precaution_id);

        Ok(precaution_id)
    }

    pub fn track_hand_hygiene_compliance(
        env: Env,
        facility_id: Address,
        unit: String,
        observation_date: u64,
        opportunities: u32,
        compliant_actions: u32,
        observer: Address,
    ) -> Result<(), Error> {
        observer.require_auth();

        if opportunities == 0 || compliant_actions > opportunities {
            return Err(Error::InvalidData);
        }

        let record_id = Self::next_id(&env, symbol_short!("hand_ctr"));
        let compliance_rate_x100 = ((i64::from(compliant_actions) * 10_000)
            / i64::from(opportunities)) as i64;

        let record = HandHygieneRecord {
            record_id,
            facility_id,
            unit,
            observation_date,
            opportunities,
            compliant_actions,
            compliance_rate_x100,
            observer,
        };

        env.storage()
            .persistent()
            .set(&DataKey::HandHygieneRecord(record_id), &record);

        Ok(())
    }

    pub fn calculate_infection_rate(
        env: Env,
        facility_id: Address,
        infection_type: Symbol,
        time_period_start: u64,
        time_period_end: u64,
        unit: Option<String>,
    ) -> Result<InfectionRate, Error> {
        if time_period_start > time_period_end {
            return Err(Error::InvalidData);
        }

        let infection_ids = Self::get_ids(&env, DataKey::InfectionIds);
        let mut numerator = 0u32;

        let mut i = 0u32;
        while i < infection_ids.len() {
            if let Some(case_id) = infection_ids.get(i) {
                if let Ok(case) = Self::get_infection_case_internal(&env, case_id) {
                    let unit_match = match &unit {
                        Some(u) => case.location == *u,
                        None => true,
                    };

                    if case.facility_id == facility_id
                        && case.infection_type == infection_type
                        && case.onset_date >= time_period_start
                        && case.onset_date <= time_period_end
                        && unit_match
                    {
                        numerator += 1;
                    }
                }
            }
            i += 1;
        }

        // Placeholder denominator until patient/device day feeds are integrated.
        let denominator = 1000u32;
        if denominator == 0 {
            return Err(Error::DivisionByZero);
        }

        let rate_per_1000_days_x100 =
            (i64::from(numerator) * 1000 * 100) / i64::from(denominator);

        Ok(InfectionRate {
            infection_type,
            numerator,
            denominator,
            rate_per_1000_days_x100,
            sir_x100: None,
        })
    }

    pub fn report_to_nhsn(
        env: Env,
        facility_id: Address,
        reporting_month: u64,
        infection_data_hash: BytesN<32>,
        device_utilization_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        let report_id = Self::next_id(&env, symbol_short!("nhsn_ctr"));
        let report = NhsnReport {
            report_id,
            facility_id,
            reporting_month,
            infection_data_hash,
            device_utilization_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::NhsnReport(report_id), &report);

        Ok(report_id)
    }

    pub fn track_antibiotic_stewardship(
        env: Env,
        facility_id: Address,
        antibiotic: String,
        days_of_therapy: u32,
        patient_days: u32,
        reporting_period: u64,
    ) -> Result<(), Error> {
        if patient_days == 0 {
            return Err(Error::DivisionByZero);
        }

        let record_id = Self::next_id(&env, symbol_short!("abx_ctr"));
        let dot_per_1000_days_x100 =
            (i64::from(days_of_therapy) * 1000 * 100) / i64::from(patient_days);

        let record = AntibioticStewardshipRecord {
            record_id,
            facility_id,
            antibiotic,
            days_of_therapy,
            patient_days,
            reporting_period,
            dot_per_1000_days_x100,
        };

        env.storage()
            .persistent()
            .set(&DataKey::StewardshipRecord(record_id), &record);

        Ok(())
    }

    pub fn alert_infection_control_team(
        env: Env,
        alert_type: Symbol,
        facility_id: Address,
        alert_details: String,
        priority: Symbol,
    ) -> Result<(), Error> {
        if !Self::is_valid_priority(&env, &priority) {
            return Err(Error::InvalidPriority);
        }

        env.events().publish(
            (Symbol::new(&env, "infection_alert"), facility_id, priority),
            (alert_type, alert_details),
        );

        Ok(())
    }

    pub fn get_infection_case(env: Env, infection_id: u64) -> Result<InfectionCase, Error> {
        Self::get_infection_case_internal(&env, infection_id)
    }

    pub fn get_active_outbreaks(env: Env, facility_id: Address) -> Vec<OutbreakCluster> {
        let outbreak_ids = Self::get_ids(&env, DataKey::OutbreakIds);
        let mut out = Vec::new(&env);

        let mut i = 0u32;
        while i < outbreak_ids.len() {
            if let Some(outbreak_id) = outbreak_ids.get(i) {
                let cluster: Option<OutbreakCluster> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::OutbreakCluster(outbreak_id));
                if let Some(cluster) = cluster {
                    if cluster.facility_id == facility_id && cluster.active {
                        out.push_back(cluster);
                    }
                }
            }
            i += 1;
        }

        out
    }

    pub fn get_active_isolations(env: Env, patient_id: Address) -> Vec<IsolationPrecaution> {
        let precaution_ids = Self::get_ids(&env, DataKey::PrecautionIds);
        let mut out = Vec::new(&env);

        let mut i = 0u32;
        while i < precaution_ids.len() {
            if let Some(precaution_id) = precaution_ids.get(i) {
                let precaution: Option<IsolationPrecaution> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::IsolationPrecaution(precaution_id));
                if let Some(precaution) = precaution {
                    if precaution.patient_id == patient_id && precaution.active {
                        out.push_back(precaution);
                    }
                }
            }
            i += 1;
        }

        out
    }

    fn get_infection_case_internal(env: &Env, infection_id: u64) -> Result<InfectionCase, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::InfectionCase(infection_id))
            .ok_or(Error::NotFound)
    }

    fn next_id(env: &Env, counter_key: Symbol) -> u64 {
        let next = env.storage().instance().get(&counter_key).unwrap_or(0u64) + 1;
        env.storage().instance().set(&counter_key, &next);
        next
    }

    fn push_id(env: &Env, list_key: DataKey, id: u64) {
        let mut ids = Self::get_ids(env, list_key.clone());
        ids.push_back(id);
        env.storage().persistent().set(&list_key, &ids);
    }

    fn get_ids(env: &Env, list_key: DataKey) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(env))
    }

    fn is_valid_infection_type(env: &Env, infection_type: &Symbol) -> bool {
        *infection_type == Symbol::new(env, "clabsi")
            || *infection_type == Symbol::new(env, "cauti")
            || *infection_type == Symbol::new(env, "ssi")
            || *infection_type == Symbol::new(env, "vap")
            || *infection_type == Symbol::new(env, "c_diff")
            || *infection_type == Symbol::new(env, "mrsa")
    }

    fn is_valid_susceptibility(env: &Env, susceptibility: &Symbol) -> bool {
        *susceptibility == Symbol::new(env, "sensitive")
            || *susceptibility == Symbol::new(env, "intermediate")
            || *susceptibility == Symbol::new(env, "resistant")
    }

    fn is_valid_precaution(env: &Env, precaution_type: &Symbol) -> bool {
        *precaution_type == Symbol::new(env, "contact")
            || *precaution_type == Symbol::new(env, "droplet")
            || *precaution_type == Symbol::new(env, "airborne")
    }

    fn is_valid_priority(env: &Env, priority: &Symbol) -> bool {
        *priority == Symbol::new(env, "low")
            || *priority == Symbol::new(env, "medium")
            || *priority == Symbol::new(env, "high")
            || *priority == Symbol::new(env, "critical")
    }
}

mod test;
