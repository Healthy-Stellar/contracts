#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

mod test;

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
/// --------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPermission {
    pub resource_id: String,
    pub granted_by: Address,
    pub granted_at: u64,
    pub expires_at: u64, // 0 means no expiration
}

/// --------------------
/// Storage Keys
/// --------------------
#[contracttype]
pub enum DataKey {
    Admin,
    Entity(Address),
    AccessList(Address),    // Entity -> Vec<AccessPermission>
    ResourceAccess(String), // Resource -> Vec<Address> (authorized parties)
}

#[contract]
pub struct AccessControl;

#[contractimpl]
impl AccessControl {
    /// Initialize the contract with an admin
    ///
    /// # Arguments
    /// * `admin` - The admin address for the contract
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);

        env.events()
            .publish((symbol_short!("init"), admin), symbol_short!("success"));
    }

    /// Register a new entity in the system
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the entity
    /// * `entity_type` - The type of entity (Hospital, Doctor, Patient, etc.)
    /// * `name` - The name of the entity
    /// * `metadata` - Additional information about the entity
    pub fn register_entity(
        env: Env,
        wallet: Address,
        entity_type: EntityType,
        name: String,
        metadata: String,
    ) {
        wallet.require_auth();

        let key = DataKey::Entity(wallet.clone());
        if env.storage().persistent().has(&key) {
            panic!("Entity already registered");
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
    }

    /// Grant access permission to an entity for a specific resource
    ///
    /// # Arguments
    /// * `grantor` - The address granting access (must be authorized)
    /// * `grantee` - The address receiving access
    /// * `resource_id` - The identifier of the resource
    /// * `expires_at` - Expiration timestamp (0 for no expiration)
    pub fn grant_access(
        env: Env,
        grantor: Address,
        grantee: Address,
        resource_id: String,
        expires_at: u64,
    ) {
        grantor.require_auth();

        // Verify grantor is a registered entity
        let grantor_key = DataKey::Entity(grantor.clone());
        if !env.storage().persistent().has(&grantor_key) {
            panic!("Grantor not registered");
        }

        // Verify grantee is a registered entity
        let grantee_key = DataKey::Entity(grantee.clone());
        if !env.storage().persistent().has(&grantee_key) {
            panic!("Grantee not registered");
        }

        let permission = AccessPermission {
            resource_id: resource_id.clone(),
            granted_by: grantor.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
        };

        // Add permission to grantee's access list
        let access_key = DataKey::AccessList(grantee.clone());
        let mut access_list: Vec<AccessPermission> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env));

        // Check if permission already exists for this resource
        let mut exists = false;
        for i in 0..access_list.len() {
            if let Some(existing) = access_list.get(i) {
                if existing.resource_id == resource_id {
                    exists = true;
                    break;
                }
            }
        }
        if exists {
            panic!("Access already granted for this resource");
        }

        access_list.push_back(permission);
        env.storage().persistent().set(&access_key, &access_list);

        // Add grantee to resource's authorized parties
        let resource_key = DataKey::ResourceAccess(resource_id.clone());
        let mut authorized: Vec<Address> = env
            .storage()
            .persistent()
            .get(&resource_key)
            .unwrap_or(Vec::new(&env));

        authorized.push_back(grantee.clone());
        env.storage().persistent().set(&resource_key, &authorized);

        env.events().publish(
            (symbol_short!("grant"), grantee, resource_id),
            symbol_short!("success"),
        );
    }

    /// Revoke access permission from an entity for a specific resource
    ///
    /// # Arguments
    /// * `revoker` - The address revoking access (must be the original grantor or admin)
    /// * `revokee` - The address losing access
    /// * `resource_id` - The identifier of the resource
    pub fn revoke_access(env: Env, revoker: Address, revokee: Address, resource_id: String) {
        revoker.require_auth();

        // Get admin for authorization check
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");

        // Remove from grantee's access list
        let access_key = DataKey::AccessList(revokee.clone());
        let access_list: Vec<AccessPermission> = env
            .storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env));

        let mut new_access_list: Vec<AccessPermission> = Vec::new(&env);
        let mut found = false;

        for i in 0..access_list.len() {
            if let Some(permission) = access_list.get(i) {
                if permission.resource_id == resource_id {
                    // Verify revoker is either the original grantor or admin
                    if permission.granted_by != revoker && revoker != admin {
                        panic!("Not authorized to revoke this access");
                    }
                    found = true;
                    // Skip this permission (effectively removing it)
                } else {
                    new_access_list.push_back(permission);
                }
            }
        }

        if !found {
            panic!("Access permission not found");
        }

        env.storage()
            .persistent()
            .set(&access_key, &new_access_list);

        // Remove from resource's authorized parties
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

        env.events().publish(
            (symbol_short!("revoke"), revokee, resource_id),
            symbol_short!("success"),
        );
    }

    /// Check if an entity has access to a specific resource
    ///
    /// # Arguments
    /// * `entity` - The address to check
    /// * `resource_id` - The identifier of the resource
    ///
    /// # Returns
    /// `true` if the entity has valid (non-expired) access, `false` otherwise
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
                    // Check if permission is expired
                    if permission.expires_at == 0 || permission.expires_at > current_time {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get all entities with access to a specific resource
    ///
    /// # Arguments
    /// * `resource_id` - The identifier of the resource
    ///
    /// # Returns
    /// A vector of addresses that have access to the resource
    pub fn get_authorized_parties(env: Env, resource_id: String) -> Vec<Address> {
        let resource_key = DataKey::ResourceAccess(resource_id);
        env.storage()
            .persistent()
            .get(&resource_key)
            .unwrap_or(Vec::new(&env))
    }

    /// Get entity details by wallet address
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the entity
    ///
    /// # Returns
    /// The EntityData for the given wallet address
    pub fn get_entity(env: Env, wallet: Address) -> EntityData {
        let key = DataKey::Entity(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .expect("Entity not found")
    }

    /// Get all access permissions for an entity
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the entity
    ///
    /// # Returns
    /// A vector of all access permissions granted to the entity
    pub fn get_entity_permissions(env: Env, wallet: Address) -> Vec<AccessPermission> {
        let access_key = DataKey::AccessList(wallet);
        env.storage()
            .persistent()
            .get(&access_key)
            .unwrap_or(Vec::new(&env))
    }

    /// Update entity metadata
    ///
    /// # Arguments
    /// * `wallet` - The wallet address of the entity
    /// * `metadata` - Updated metadata information
    pub fn update_entity(env: Env, wallet: Address, metadata: String) {
        wallet.require_auth();

        let key = DataKey::Entity(wallet.clone());
        let mut entity: EntityData = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Entity not found");

        entity.metadata = metadata;
        env.storage().persistent().set(&key, &entity);

        env.events()
            .publish((symbol_short!("upd_ent"), wallet), symbol_short!("success"));
    }

    /// Deactivate an entity (admin only)
    ///
    /// # Arguments
    /// * `admin` - The admin address
    /// * `wallet` - The wallet address of the entity to deactivate
    pub fn deactivate_entity(env: Env, admin: Address, wallet: Address) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");

        if admin != stored_admin {
            panic!("Only admin can deactivate entities");
        }

        let key = DataKey::Entity(wallet.clone());
        let mut entity: EntityData = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Entity not found");

        entity.active = false;
        env.storage().persistent().set(&key, &entity);

        env.events()
            .publish((symbol_short!("deact"), wallet), symbol_short!("success"));
    }
}
