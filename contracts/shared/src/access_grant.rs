use soroban_sdk::{Address, Env, IntoVal, Val, Vec};

/// Grant a subject access and add it to the patient's grant index.
///
/// The caller supplies its own storage keys, allowing this helper to be used
/// with contract-specific `DataKey` enums.
pub fn grant_access<G, I>(env: &Env, grant_key: &G, index_key: &I, subject: &Address)
where
    G: IntoVal<Env, Val>,
    I: IntoVal<Env, Val>,
{
    env.storage().persistent().set(grant_key, &true);

    let mut subjects: Vec<Address> = env
        .storage()
        .persistent()
        .get(index_key)
        .unwrap_or(Vec::new(env));
    if !subjects.iter().any(|existing| existing == *subject) {
        subjects.push_back(subject.clone());
        env.storage().persistent().set(index_key, &subjects);
    }
}

/// Revoke a subject's access and remove it from the patient's grant index.
pub fn revoke_access<G, I>(env: &Env, grant_key: &G, index_key: &I, subject: &Address)
where
    G: IntoVal<Env, Val>,
    I: IntoVal<Env, Val>,
{
    env.storage().persistent().remove(grant_key);

    let subjects: Vec<Address> = env
        .storage()
        .persistent()
        .get(index_key)
        .unwrap_or(Vec::new(env));
    let mut updated = Vec::new(env);
    for existing in subjects.iter() {
        if existing != *subject {
            updated.push_back(existing);
        }
    }
    env.storage().persistent().set(index_key, &updated);
}

/// Check whether the grant key is present in persistent storage.
pub fn has_access<K>(env: &Env, grant_key: &K) -> bool
where
    K: IntoVal<Env, Val>,
{
    env.storage().persistent().has(grant_key)
}

/// Remove every grant listed under `index_key`, then remove the index itself.
pub fn remove_all_access_grants<G, I, F>(env: &Env, index_key: &I, grant_key_for: F)
where
    G: IntoVal<Env, Val>,
    I: IntoVal<Env, Val>,
    F: Fn(&Address) -> G,
{
    let subjects: Vec<Address> = env
        .storage()
        .persistent()
        .get(index_key)
        .unwrap_or(Vec::new(env));
    for subject in subjects.iter() {
        env.storage()
            .persistent()
            .remove(&grant_key_for(&subject));
    }
    env.storage().persistent().remove(index_key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, contracttype, testutils::Address as _};

    #[contracttype]
    #[derive(Clone)]
    enum TestKey {
        Grant(Address, Address),
        Index(Address),
    }

    #[contract]
    struct DummyContract;

    #[contractimpl]
    impl DummyContract {
        pub fn noop() {}
    }

    fn with_contract<T>(env: &Env, contract: &Address, f: impl FnOnce() -> T) -> T {
        env.as_contract(contract, f)
    }

    #[test]
    fn grants_are_idempotent_and_revoke_updates_index() {
        let env = Env::default();
        let contract = env.register(DummyContract, ());
        let patient = Address::generate(&env);
        let first = Address::generate(&env);
        let second = Address::generate(&env);
        let index_key = TestKey::Index(patient.clone());

        with_contract(&env, &contract, || {
            grant_access(
                &env,
                &TestKey::Grant(patient.clone(), first.clone()),
                &index_key,
                &first,
            );
            grant_access(
                &env,
                &TestKey::Grant(patient.clone(), first.clone()),
                &index_key,
                &first,
            );
            grant_access(
                &env,
                &TestKey::Grant(patient.clone(), second.clone()),
                &index_key,
                &second,
            );

            let subjects: Vec<Address> = env
                .storage()
                .persistent()
                .get(&index_key)
                .unwrap();
            assert_eq!(subjects.len(), 2);
            assert!(has_access(
                &env,
                &TestKey::Grant(patient.clone(), first.clone())
            ));

            revoke_access(
                &env,
                &TestKey::Grant(patient.clone(), first.clone()),
                &index_key,
                &first,
            );
            assert!(!has_access(
                &env,
                &TestKey::Grant(patient.clone(), first.clone())
            ));
            let subjects: Vec<Address> = env
                .storage()
                .persistent()
                .get(&index_key)
                .unwrap();
            assert_eq!(subjects.len(), 1);
            assert_eq!(subjects.get(0), Some(second.clone()));
        });
    }

    #[test]
    fn remove_all_clears_grants_and_index() {
        let env = Env::default();
        let contract = env.register(DummyContract, ());
        let patient = Address::generate(&env);
        let first = Address::generate(&env);
        let second = Address::generate(&env);
        let index_key = TestKey::Index(patient.clone());

        with_contract(&env, &contract, || {
            grant_access(
                &env,
                &TestKey::Grant(patient.clone(), first.clone()),
                &index_key,
                &first,
            );
            grant_access(
                &env,
                &TestKey::Grant(patient.clone(), second.clone()),
                &index_key,
                &second,
            );
            remove_all_access_grants(&env, &index_key, |subject| {
                TestKey::Grant(patient.clone(), subject.clone())
            });

            assert!(!has_access(
                &env,
                &TestKey::Grant(patient.clone(), first)
            ));
            assert!(!has_access(
                &env,
                &TestKey::Grant(patient.clone(), second)
            ));
            assert!(!env.storage().persistent().has(&index_key));
        });
    }
}