#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, xdr::ToXdr, Address, Bytes,
    BytesN, Env, String, Vec,
};

mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    InvalidDidFormat = 1,
    AlreadyInitialized = 2,
    EntityAlreadyRegistered = 3,
    EntityNotFound = 4,
    GrantorNotRegistered = 5,
    GranteeNotRegistered = 6,
    AccessAlreadyGranted = 7,
    NotAuthorizedToRevoke = 8,
    AccessPermissionNotFound = 9,
    ContractNotInitialized = 10,
    OnlyAdminCanDeactivate = 11,
    // #228: commit-reveal
    CommitNotFound = 12,
    CommitHashMismatch = 13,
    CommitAlreadyUsed = 14,
    // #223: unified consent engine
    ConsentNotFound = 15,
    ConsentExpired = 16,
    ConsentRevoked = 17,
    ConsentDenied = 18,
    InvalidScopeMask = 19,
    InsufficientRole = 20,
    RoleAlreadyGranted = 21,
    RoleNotFound = 22,
    RateLimitExceeded = 23,
}

/// --------------------
/// Role Types (RBAC)
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Admin,
    Doctor,
    Nurse,
    Patient,
    Insurer,
    Auditor,
    Provider,
    PayerReviewer,
    EmergencyResponder,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleAssignment {
    pub granted_by: Address,
    pub granted_at: u64,
    pub expires_at: u64,
}

/// --------------------
/// Entity Types
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityType {
    Hospital,
    Doctor,
    Patient,
    Insurer,
    Admin,
}

/// --------------------
/// Entity Data
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityData {
    pub entity_type: EntityType,
    pub name: String,
    pub metadata: String,
    pub active: bool,
}

/// --------------------
/// Access Permission
/// #222: op_id added for correlation / forensic auditability
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPermission {
    pub resource_id: String,
    pub granted_by: Address,
    pub granted_at: u64,
    pub expires_at: u64, // 0 means no expiration
    pub op_id: u64,      // #222: immutable operation receipt / correlation ID
}

/// --------------------
/// #223: Unified Consent Record
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsentStatus {
    Active,
    Revoked,
    Expired,
}

/// Index entry for subject's consent list.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentIndexEntry {
    pub grantee: Address,
    pub purpose_code: String,
}

/// A structured consent object capturing all HIPAA/GDPR-relevant semantics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsentRecord {
    /// The data subject (patient) granting consent.
    pub subject: Address,
    /// The party receiving access (provider, researcher, etc.).
    pub grantee: Address,
    /// Bitmask of permitted operations (e.g. 0x01=read, 0x02=write, 0x04=share).
    pub scope_mask: u32,
    /// Purpose code (e.g. "treatment", "research", "billing").
    pub purpose_code: String,
    /// Legal basis (e.g. "explicit_consent", "vital_interest", "legal_obligation").
    pub legal_basis: String,
    /// Unix timestamp when consent was granted.
    pub granted_at: u64,
    /// Unix timestamp when consent expires; 0 = no expiry.
    pub expires_at: u64,
    /// Current status.
    pub status: ConsentStatus,
    /// Monotonic operation ID for auditability.
    pub op_id: u64,
}

/// --------------------
/// #228: Pending commit for commit-reveal anti-front-running
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingCommit {
    pub committer: Address,
    pub committed_at: u64,
    pub used: bool,
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Admin,
    Entity(Address),
    AccessList(Address),                    // Entity -> Vec<AccessPermission>
    ResourceAccess(String),                 // Resource -> Vec<Address> (authorized parties)
    Did(Address),
    // #220: composite uniqueness index: (grantor, grantee, resource) -> bool
    GrantIndex(Address, Address, String),
    // #222: monotonic operation counter
    OpCounter,
    // #228: commit-reveal: hash -> PendingCommit
    Commit(BytesN<32>),
    // #223: unified consent engine
    // (subject, grantee, purpose_code) -> ConsentRecord
    Consent(Address, Address, String),
    // subject -> Vec<(grantee, purpose_code)> for enumeration
    SubjectConsents(Address),
    // RBAC: (address, role) -> RoleAssignment
    RoleAssignment(Address, Role),
    // Rate limiting: (address, ledger_sequence) -> u32 count
    RateLimit(Address, u32),
}

#[contract]
pub struct AccessControl;

#[contractimpl]
impl AccessControl {
    // -------------------------------------------------------------------------
    // Rate limiting
    // -------------------------------------------------------------------------

    const MAX_CONSENT_OPS_PER_BLOCK: u32 = 10;

    /// Increment the per-caller per-block consent operation counter and return
    /// `RateLimitExceeded` if the limit is breached.
    fn check_rate_limit(env: &Env, caller: &Address) -> Result<(), ContractError> {
        let seq = env.ledger().sequence();
        let key = DataKey::RateLimit(caller.clone(), seq);
        let count: u32 = env.storage().temporary().get(&key).unwrap_or(0);
        if count >= Self::MAX_CONSENT_OPS_PER_BLOCK {
            return Err(ContractError::RateLimitExceeded);
        }
        // TTL of 1 ledger is enough — the entry is only meaningful for the
        // current sequence number.
        env.storage().temporary().set(&key, &(count + 1));
        env.storage().temporary().extend_ttl(&key, 1, 1);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Internal role helpers
    // -------------------------------------------------------------------------

    /// Returns the stored `RoleAssignment` for `(address, role)` if it exists
    /// **and** has not expired. Expired entries are treated as absent.
    fn load_active_role(
        env: &Env,
        address: &Address,
        role: &Role,
    ) -> Option<RoleAssignment> {
        let key = DataKey::RoleAssignment(address.clone(), role.clone());
        let assignment: RoleAssignment = env.storage().persistent().get(&key)?;
        let now = env.ledger().timestamp();
        if assignment.expires_at != 0 && assignment.expires_at <= now {
            return None;
        }
        Some(assignment)
    }

    /// Asserts that `caller` holds `role` (and the role has not expired).
    /// Returns `InsufficientRole` if the check fails.
    fn require_role(
        env: &Env,
        caller: &Address,
        role: &Role,
    ) -> Result<(), ContractError> {
        // Admin always satisfies any role check.
        let admin_opt: Option<Address> = env.storage().persistent().get(&DataKey::Admin);
        if let Some(ref admin) = admin_opt {
            if caller == admin {
                return Ok(());
            }
        }
        if Self::load_active_role(env, caller, role).is_some() {
            return Ok(());
        }
        Err(ContractError::InsufficientRole)
    }

    // -------------------------------------------------------------------------
    // Initialize
    // -------------------------------------------------------------------------

    /// Initialize the contract with an admin
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);

        // Bootstrap: give the admin the Admin role so role checks are uniform.
        let bootstrap = RoleAssignment {
            granted_by: admin.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at: 0,
        };
        env.storage().persistent().set(
            &DataKey::RoleAssignment(admin.clone(), Role::Admin),
            &bootstrap,
        );

        env.events()
            .publish((symbol_short!("init"), admin), symbol_short!("success"));
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Role management
    // -------------------------------------------------------------------------

    /// Grant `role` to `grantee`. Only an address that itself holds the
    /// `Admin` role (or is the stored admin address) may call this.
    ///
    /// # Arguments
    /// * `granter`    - Must hold the `Admin` role.
    /// * `grantee`    - Address receiving the role.
    /// * `role`       - The role to grant.
    /// * `expires_at` - Expiry timestamp; pass `0` for no expiry.
    pub fn grant_role(
        env: Env,
        granter: Address,
        grantee: Address,
        role: Role,
        expires_at: u64,
    ) -> Result<(), ContractError> {
        granter.require_auth();
        Self::require_role(&env, &granter, &Role::Admin)?;

        let key = DataKey::RoleAssignment(grantee.clone(), role.clone());
        if env.storage().persistent().has(&key) {
            // Allow re-grant only if the existing assignment has expired.
            if Self::load_active_role(&env, &grantee, &role).is_some() {
                return Err(ContractError::RoleAlreadyGranted);
            }
        }

        let assignment = RoleAssignment {
            granted_by: granter.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
        };
        env.storage().persistent().set(&key, &assignment);

        env.events().publish(
            (symbol_short!("role_grt"), grantee, role),
            symbol_short!("success"),
        );
        Ok(())
    }

    /// Revoke `role` from `revokee`. Only an address that holds the `Admin`
    /// role may call this.
    ///
    /// # Arguments
    /// * `revoker`  - Must hold the `Admin` role.
    /// * `revokee`  - Address losing the role.
    /// * `role`     - The role to revoke.
    pub fn revoke_role(
        env: Env,
        revoker: Address,
        revokee: Address,
        role: Role,
    ) -> Result<(), ContractError> {
        revoker.require_auth();
        Self::require_role(&env, &revoker, &Role::Admin)?;

        let key = DataKey::RoleAssignment(revokee.clone(), role.clone());
        if !env.storage().persistent().has(&key) {
            return Err(ContractError::RoleNotFound);
        }
        env.storage().persistent().remove(&key);

        env.events().publish(
            (symbol_short!("role_rev"), revokee, role),
            symbol_short!("success"),
        );
        Ok(())
    }

    /// Returns `true` if `address` currently holds `role` (and it has not
    /// expired). Does **not** require any auth — safe to call as a view.
    pub fn has_role(env: Env, address: Address, role: Role) -> bool {
        // Admin address always satisfies any role.
        let admin_opt: Option<Address> = env.storage().persistent().get(&DataKey::Admin);
        if let Some(ref admin) = admin_opt {
            if &address == admin {
                return true;
            }
        }
        Self::load_active_role(&env, &address, &role).is_some()
    }

    /// Returns the full `RoleAssignment` for `(address, role)`, or an error
    /// if the role was never granted or has expired.
    pub fn get_role_assignment(
        env: Env,
        address: Address,
        role: Role,
    ) -> Result<RoleAssignment, ContractError> {
        Self::load_active_role(&env, &address, &role)
            .ok_or(ContractError::RoleNotFound)
    }

    /// Register a new entity in the system
    pub fn register_entity(
        env: Env,
        wallet: Address,
        entity_type: EntityType,
        name: String,
        metadata: String,
    ) -> Result<(), ContractError> {
        wallet.require_auth();

        let key = DataKey::Entity(wallet.clone());
        if env.storage().persistent().has(&key) {
            return Err(ContractError::EntityAlreadyRegistered);
        }

        let entity = EntityData {
            entity_type,
            name,
            metadata,
            active: true,
        };

        env.storage().persistent().set(&key, &entity);

        // Initialize empty access list for the entity
        let empty_access: Vec<AccessPermission> = Vec::new(&env);
        env.storage()
            .persistent()
            .set(&DataKey::AccessList(wallet.clone()), &empty_access);

        env.events()
            .publish((symbol_short!("reg_ent"), wallet), symbol_short!("success"));
        Ok(())
    }

    // -----------------------------------------------------------------------
    // #228: Commit phase — caller submits hash(nonce || grantor || grantee ||
    //       resource_id) before the reveal (grant_access) call.
    // -----------------------------------------------------------------------
    /// Commit a hash before calling grant_access to prevent front-running.
    ///
    /// # Arguments
    /// * `committer` - The address that will later call grant_access
    /// * `commit_hash` - sha256(nonce || grantor || grantee || resource_id)
    pub fn commit_grant(
        env: Env,
        committer: Address,
        commit_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        committer.require_auth();

        let key = DataKey::Commit(commit_hash.clone());
        // Reject re-use of the same hash
        if env.storage().temporary().has(&key) {
            return Err(ContractError::CommitAlreadyUsed);
        }

        let commit = PendingCommit {
            committer: committer.clone(),
            committed_at: env.ledger().timestamp(),
            used: false,
        };
        // Store with a TTL of ~1 hour (3600 ledgers at ~1s each)
        env.storage().temporary().set(&key, &commit);
        env.storage()
            .temporary()
            .extend_ttl(&key, 3600, 3600);

        env.events()
            .publish((symbol_short!("committed"), committer), commit_hash);
        Ok(())
    }

    /// Grant access permission to an entity for a specific resource.
    ///
    /// For sensitive operations, callers should first call `commit_grant` with
    /// hash(nonce || grantor || grantee || resource_id) and pass the same
    /// `nonce` here so the contract can verify the commit (anti-front-running).
    ///
    /// Pass `nonce = None` to skip commit-reveal verification (backward-compat).
    ///
    /// # Arguments
    /// * `grantor`      - The address granting access (must be authorized)
    /// * `grantee`      - The address receiving access
    /// * `resource_id`  - The identifier of the resource
    /// * `expires_at`   - Expiration timestamp (0 for no expiration)
    /// * `nonce`        - Optional nonce used in commit_grant
    pub fn grant_access(
        env: Env,
        grantor: Address,
        grantee: Address,
        resource_id: String,
        expires_at: u64,
        nonce: Option<BytesN<32>>,
    ) -> Result<u64, ContractError> {
        grantor.require_auth();

        // #228: verify commit if nonce provided
        if let Some(n) = nonce {
            Self::verify_and_consume_commit(&env, &grantor, &grantee, &resource_id, n)?;
        }

        // Verify grantor is a registered entity
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Entity(grantor.clone()))
        {
            return Err(ContractError::GrantorNotRegistered);
        }

        // Verify grantee is a registered entity
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Entity(grantee.clone()))
        {
            return Err(ContractError::GranteeNotRegistered);
        }

        // #220: composite uniqueness check — (grantor, grantee, resource_id)
        let grant_idx = DataKey::GrantIndex(
            grantor.clone(),
            grantee.clone(),
            resource_id.clone(),
        );
        if env.storage().persistent().has(&grant_idx) {
            return Err(ContractError::AccessAlreadyGranted);
        }

        // #222: assign monotonic operation ID
        let op_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::OpCounter)
            .unwrap_or(0u64)
            + 1;
        env.storage().instance().set(&DataKey::OpCounter, &op_id);

        let permission = AccessPermission {
            resource_id: resource_id.clone(),
            granted_by: grantor.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
            op_id,
        };

        // Add permission to grantee's access list
        let access_key = DataKey::AccessList(grantee.clone());
        let mut access_list: Vec<AccessPermission> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env));

        access_list.push_back(permission);
        env.storage().persistent().set(&access_key, &access_list);

        // #220: record composite grant index
        env.storage().persistent().set(&grant_idx, &true);

        // #224: add grantee to resource's authorized parties (symmetric index)
        let resource_key = DataKey::ResourceAccess(resource_id.clone());
        let mut authorized: Vec<Address> = env
            .storage()
            .persistent()
            .get(&resource_key)
            .unwrap_or(Vec::new(&env));
        authorized.push_back(grantee.clone());
        env.storage().persistent().set(&resource_key, &authorized);

        // #222: include op_id in event for correlation
        env.events().publish(
            (symbol_short!("grant"), grantee, resource_id),
            op_id,
        );
        Ok(op_id)
    }

    /// Revoke access permission from an entity for a specific resource.
    ///
    /// Atomically removes the permission from ALL indexes:
    ///   1. grantee's AccessList
    ///   2. resource's ResourceAccess list
    ///   3. composite GrantIndex
    ///
    /// # Arguments
    /// * `revoker`     - The address revoking access (must be the original grantor or admin)
    /// * `revokee`     - The address losing access
    /// * `resource_id` - The identifier of the resource
    pub fn revoke_access(
        env: Env,
        revoker: Address,
        revokee: Address,
        resource_id: String,
    ) -> Result<u64, ContractError> {
        revoker.require_auth();

        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ContractError::ContractNotInitialized)?;

        // --- Step 1: remove from grantee's access list, capture grantor ---
        let access_key = DataKey::AccessList(revokee.clone());
        let access_list: Vec<AccessPermission> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env));

        let mut new_access_list: Vec<AccessPermission> = Vec::new(&env);
        let mut found_grantor: Option<Address> = None;
        let mut revoked_op_id: u64 = 0;

        for i in 0..access_list.len() {
            if let Some(permission) = access_list.get(i) {
                if permission.resource_id == resource_id && found_grantor.is_none() {
                    // Verify revoker is either the original grantor, admin, or holds PayerReviewer role
                    let is_grantor = permission.granted_by == revoker;
                    let is_admin = revoker == admin;
                    let is_payer_reviewer =
                        Self::load_active_role(&env, &revoker, &Role::PayerReviewer).is_some();
                    if !is_grantor && !is_admin && !is_payer_reviewer {
                        return Err(ContractError::NotAuthorizedToRevoke);
                    }
                    found_grantor = Some(permission.granted_by.clone());
                    revoked_op_id = permission.op_id;
                    // skip — effectively removing it
                } else {
                    new_access_list.push_back(permission);
                }
            }
        }

        let grantor = found_grantor.ok_or(ContractError::AccessPermissionNotFound)?;

        env.storage()
            .persistent()
            .set(&access_key, &new_access_list);

        // --- Step 2: remove from resource's authorized parties (#224 atomic) ---
        let resource_key = DataKey::ResourceAccess(resource_id.clone());
        let authorized: Vec<Address> = env
            .storage()
            .persistent()
            .get(&resource_key)
            .unwrap_or(Vec::new(&env));

        let mut new_authorized: Vec<Address> = Vec::new(&env);
        for i in 0..authorized.len() {
            if let Some(addr) = authorized.get(i) {
                if addr != revokee {
                    new_authorized.push_back(addr);
                }
            }
        }
        env.storage()
            .persistent()
            .set(&resource_key, &new_authorized);

        // --- Step 3: remove composite grant index (#220 + #224 symmetric) ---
        let grant_idx = DataKey::GrantIndex(
            grantor,
            revokee.clone(),
            resource_id.clone(),
        );
        env.storage().persistent().remove(&grant_idx);

        // #222: assign op_id for the revocation event
        let op_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::OpCounter)
            .unwrap_or(0u64)
            + 1;
        env.storage().instance().set(&DataKey::OpCounter, &op_id);

        // #222: include both the revocation op_id and the original grant op_id
        env.events().publish(
            (symbol_short!("revoke"), revokee, resource_id),
            (op_id, revoked_op_id),
        );
        Ok(op_id)
    }

    /// Check if an entity has access to a specific resource
    pub fn check_access(env: Env, entity: Address, resource_id: String) -> bool {
        let access_key = DataKey::AccessList(entity);
        let access_list: Vec<AccessPermission> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env));

        let current_time = env.ledger().timestamp();

        for i in 0..access_list.len() {
            if let Some(permission) = access_list.get(i) {
                if permission.resource_id == resource_id {
                    if permission.expires_at == 0 || permission.expires_at > current_time {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get all entities with access to a specific resource
    pub fn get_authorized_parties(env: Env, resource_id: String) -> Vec<Address> {
        let resource_key = DataKey::ResourceAccess(resource_id);
        env.storage()
            .persistent()
            .get(&resource_key)
            .unwrap_or(Vec::new(&env))
    }

    /// Get entity details by wallet address
    pub fn get_entity(env: Env, wallet: Address) -> Result<EntityData, ContractError> {
        let key = DataKey::Entity(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::EntityNotFound)
    }

    /// Get all access permissions for an entity
    pub fn get_entity_permissions(env: Env, wallet: Address) -> Vec<AccessPermission> {
        let access_key = DataKey::AccessList(wallet);
        env.storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env))
    }

    /// Update entity metadata
    pub fn update_entity(env: Env, wallet: Address, metadata: String) -> Result<(), ContractError> {
        wallet.require_auth();

        let key = DataKey::Entity(wallet.clone());
        let mut entity: EntityData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::EntityNotFound)?;

        entity.metadata = metadata;
        env.storage().persistent().set(&key, &entity);

        env.events()
            .publish((symbol_short!("upd_ent"), wallet), symbol_short!("success"));
        Ok(())
    }

    /// Deactivate an entity (admin only).
    ///
    /// In addition to marking the entity inactive, this function emits an
    /// `acc_rev` event for every active `AccessPermission` held by the entity
    /// and a `cst_rev` event for every active `ConsentRecord` where the entity
    /// is the subject.  External audit systems can therefore treat a single
    /// deactivation as an implicit revocation of all access without needing a
    /// separate revocation pass.
    pub fn deactivate_entity(
        env: Env,
        admin: Address,
        wallet: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(ContractError::ContractNotInitialized)?;

        if admin != stored_admin {
            return Err(ContractError::OnlyAdminCanDeactivate);
        }

        let key = DataKey::Entity(wallet.clone());
        let mut entity: EntityData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::EntityNotFound)?;

        entity.active = false;
        env.storage().persistent().set(&key, &entity);

        // ── Emit one access-revoked event per active AccessPermission ─────────
        // We read the entity's access list and emit an `acc_rev` event for each
        // entry so that audit loggers receive an explicit signal for every
        // implicit revocation caused by deactivation.
        let access_key = DataKey::AccessList(wallet.clone());
        let access_list: Vec<AccessPermission> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env));

        let now = env.ledger().timestamp();
        for i in 0..access_list.len() {
            if let Some(permission) = access_list.get(i) {
                // Only emit for non-expired permissions — expired ones were
                // already effectively inactive.
                let is_active =
                    permission.expires_at == 0 || permission.expires_at > now;
                if is_active {
                    env.events().publish(
                        (symbol_short!("acc_rev"), wallet.clone(), permission.resource_id),
                        permission.op_id,
                    );
                }
            }
        }

        // ── Emit one consent-revoked event per active ConsentRecord ───────────
        // Walk the subject's consent index and emit a `cst_rev` event for every
        // consent that is still Active (and not expired).
        let idx_key = DataKey::SubjectConsents(wallet.clone());
        let consent_index: Vec<ConsentIndexEntry> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));

        for i in 0..consent_index.len() {
            if let Some(entry) = consent_index.get(i) {
                let consent_key = DataKey::Consent(
                    wallet.clone(),
                    entry.grantee.clone(),
                    entry.purpose_code.clone(),
                );
                if let Some(record) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, ConsentRecord>(&consent_key)
                {
                    let is_active = matches!(record.status, ConsentStatus::Active)
                        && (record.expires_at == 0 || record.expires_at > now);
                    if is_active {
                        env.events().publish(
                            (symbol_short!("cst_rev"), wallet.clone(), entry.grantee),
                            (entry.purpose_code, record.op_id),
                        );
                    }
                }
            }
        }

        // ── Emit the top-level deactivation event ─────────────────────────────
        // Downstream consumers that prefer a single "deactivate = revoke all"
        // signal can listen for this event alone.
        env.events()
            .publish((symbol_short!("deact"), wallet), symbol_short!("success"));
        Ok(())
    }

    /// Register or update a W3C DID for the provided address.
    pub fn register_did(env: Env, address: Address, did: Bytes) -> Result<(), ContractError> {
        address.require_auth();
        Self::validate_did(&did)?;

        let key = DataKey::Did(address.clone());
        let old_did: Option<Bytes> = env.storage().persistent().get(&key);
        let old_hash: Option<BytesN<32>> = old_did.map(|d| env.crypto().sha256(&d).into());
        let new_hash: BytesN<32> = env.crypto().sha256(&did).into();

        env.storage().persistent().set(&key, &did);
        env.events()
            .publish((symbol_short!("did_aud"), address), (old_hash, new_hash));
        Ok(())
    }

    /// Returns the DID registered for an address, if present.
    pub fn get_did(env: Env, address: Address) -> Option<Bytes> {
        env.storage().persistent().get(&DataKey::Did(address))
    }

    // -----------------------------------------------------------------------
    // #223: Unified Consent Engine
    // -----------------------------------------------------------------------

    /// Grant structured consent from a subject (patient) to a grantee.
    ///
    /// # Arguments
    /// * `subject`      - The data subject granting consent (must auth)
    /// * `grantee`      - The party receiving access
    /// * `scope_mask`   - Bitmask: 0x01=read, 0x02=write, 0x04=share
    /// * `purpose_code` - e.g. "treatment", "research", "billing"
    /// * `legal_basis`  - e.g. "explicit_consent", "vital_interest"
    /// * `expires_at`   - Unix timestamp; 0 = no expiry
    pub fn grant_consent(
        env: Env,
        subject: Address,
        grantee: Address,
        scope_mask: u32,
        purpose_code: String,
        legal_basis: String,
        expires_at: u64,
    ) -> Result<u64, ContractError> {
        subject.require_auth();

        Self::check_rate_limit(&env, &subject)?;

        if scope_mask == 0 {
            return Err(ContractError::InvalidScopeMask);
        }

        let op_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::OpCounter)
            .unwrap_or(0u64)
            + 1;
        env.storage().instance().set(&DataKey::OpCounter, &op_id);

        let record = ConsentRecord {
            subject: subject.clone(),
            grantee: grantee.clone(),
            scope_mask,
            purpose_code: purpose_code.clone(),
            legal_basis,
            granted_at: env.ledger().timestamp(),
            expires_at,
            status: ConsentStatus::Active,
            op_id,
        };

        let key = DataKey::Consent(subject.clone(), grantee.clone(), purpose_code.clone());
        env.storage().persistent().set(&key, &record);

        // Update subject's consent index.
        let idx_key = DataKey::SubjectConsents(subject.clone());
        let mut index: Vec<ConsentIndexEntry> = env
            .storage()
            .persistent()
            .get(&idx_key)
            .unwrap_or(Vec::new(&env));
        // Replace existing entry for same (grantee, purpose) if present.
        let mut found = false;
        for i in 0..index.len() {
            if let Some(entry) = index.get(i) {
                if entry.grantee == grantee && entry.purpose_code == purpose_code {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            index.push_back(ConsentIndexEntry {
                grantee: grantee.clone(),
                purpose_code: purpose_code.clone(),
            });
            env.storage().persistent().set(&idx_key, &index);
        }

        env.events().publish(
            (symbol_short!("consent"), subject, grantee),
            (purpose_code, scope_mask, op_id),
        );

        Ok(op_id)
    }

    /// Revoke a previously granted consent.
    pub fn revoke_consent(
        env: Env,
        subject: Address,
        grantee: Address,
        purpose_code: String,
    ) -> Result<u64, ContractError> {
        subject.require_auth();

        Self::check_rate_limit(&env, &subject)?;

        let key = DataKey::Consent(subject.clone(), grantee.clone(), purpose_code.clone());
        let mut record: ConsentRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::ConsentNotFound)?;

        record.status = ConsentStatus::Revoked;
        env.storage().persistent().set(&key, &record);

        let op_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::OpCounter)
            .unwrap_or(0u64)
            + 1;
        env.storage().instance().set(&DataKey::OpCounter, &op_id);

        env.events().publish(
            (symbol_short!("rev_cst"), subject, grantee),
            (purpose_code, op_id),
        );

        Ok(op_id)
    }

    /// Check whether active consent exists for (subject, grantee, purpose_code)
    /// and that the requested scope bits are all covered.
    /// Returns `Ok(())` if access is permitted, or an error otherwise.
    pub fn check_consent(
        env: Env,
        subject: Address,
        grantee: Address,
        purpose_code: String,
        required_scope: u32,
    ) -> Result<(), ContractError> {
        let key = DataKey::Consent(subject, grantee, purpose_code);
        let record: ConsentRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::ConsentNotFound)?;

        match record.status {
            ConsentStatus::Revoked => return Err(ContractError::ConsentRevoked),
            ConsentStatus::Expired => return Err(ContractError::ConsentExpired),
            ConsentStatus::Active => {}
        }

        let now = env.ledger().timestamp();
        if record.expires_at != 0 && now > record.expires_at {
            return Err(ContractError::ConsentExpired);
        }

        if required_scope != 0 && (record.scope_mask & required_scope) != required_scope {
            return Err(ContractError::ConsentDenied);
        }

        Ok(())
    }

    /// Return the full consent record for a (subject, grantee, purpose_code) triple.
    pub fn get_consent(
        env: Env,
        subject: Address,
        grantee: Address,
        purpose_code: String,
    ) -> Result<ConsentRecord, ContractError> {
        env.storage()
            .persistent()
            .get(&DataKey::Consent(subject, grantee, purpose_code))
            .ok_or(ContractError::ConsentNotFound)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn validate_did(did: &Bytes) -> Result<(), ContractError> {
        if did.len() < 4 {
            return Err(ContractError::InvalidDidFormat);
        }
        let d = did.get(0).unwrap_or_default();
        let i = did.get(1).unwrap_or_default();
        let d2 = did.get(2).unwrap_or_default();
        let colon = did.get(3).unwrap_or_default();
        if d != b'd' || i != b'i' || d2 != b'd' || colon != b':' {
            return Err(ContractError::InvalidDidFormat);
        }
        Ok(())
    }

    /// #228: Verify that a valid commit exists for (grantor, grantee, resource_id, nonce)
    /// and mark it as used.
    fn verify_and_consume_commit(
        env: &Env,
        grantor: &Address,
        grantee: &Address,
        resource_id: &String,
        nonce: BytesN<32>,
    ) -> Result<(), ContractError> {
        // Reconstruct the expected hash: sha256(nonce || grantor_xdr || grantee_xdr || resource_xdr)
        let mut data = Bytes::new(env);
        data.append(&nonce.clone().into());
        data.append(&grantor.clone().to_xdr(env));
        data.append(&grantee.clone().to_xdr(env));
        data.append(&resource_id.clone().to_xdr(env));
        let expected_hash: BytesN<32> = env.crypto().sha256(&data).into();

        let key = DataKey::Commit(expected_hash.clone());
        let mut commit: PendingCommit = env
            .storage()
            .temporary()
            .get(&key)
            .ok_or(ContractError::CommitNotFound)?;

        if commit.used {
            return Err(ContractError::CommitAlreadyUsed);
        }

        commit.used = true;
        env.storage().temporary().set(&key, &commit);
        Ok(())
    }
}
