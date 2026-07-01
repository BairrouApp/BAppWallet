use crate::types::{Campaign, Coupon, DataKey, Error};
use soroban_sdk::{Address, Env, Vec};

pub fn get_campaign(env: &Env) -> Result<Campaign, Error> {
    if let Some(campaign) = env.storage().instance().get(&DataKey::Campaign) {
        Ok(campaign)
    } else {
        Err(Error::NotInitialized)
    }
}

pub fn set_campaign(env: &Env, campaign: &Campaign) {
    env.storage().instance().set(&DataKey::Campaign, campaign);
    extend_instance_ttl(env);
}

pub fn get_coupon(env: &Env, coupon_id: u32) -> Option<Coupon> {
    env.storage().persistent().get(&DataKey::Coupon(coupon_id))
}

pub fn set_coupon(env: &Env, coupon_id: u32, coupon: &Coupon) {
    let key = DataKey::Coupon(coupon_id);
    env.storage().persistent().set(&key, coupon);
    extend_persistent_ttl(env, &key);
}

pub fn remove_coupon(env: &Env, coupon_id: u32) {
    let key = DataKey::Coupon(coupon_id);
    if env.storage().persistent().has(&key) {
        env.storage().persistent().remove(&key);
    }
}

pub fn get_claimed_coupons(env: &Env, user: &Address) -> Vec<u32> {
    env.storage().persistent().get(&DataKey::Claimed(user.clone())).unwrap_or_else(|| Vec::new(env))
}

pub fn add_claimed_coupon(env: &Env, user: &Address, coupon_id: u32) {
    let key = DataKey::Claimed(user.clone());
    let mut coupons = get_claimed_coupons(env, user);
    coupons.push_back(coupon_id);
    env.storage().persistent().set(&key, &coupons);
    extend_persistent_ttl(env, &key);
}

pub fn is_redeemer(env: &Env, partner: &Address) -> bool {
    env.storage().persistent().has(&DataKey::AuthorizedRedeemer(partner.clone()))
}

pub fn set_redeemer(env: &Env, partner: &Address) {
    let key = DataKey::AuthorizedRedeemer(partner.clone());
    env.storage().persistent().set(&key, &true);
    extend_persistent_ttl(env, &key);
}

pub fn remove_redeemer(env: &Env, partner: &Address) {
    let key = DataKey::AuthorizedRedeemer(partner.clone());
    if env.storage().persistent().has(&key) {
        env.storage().persistent().remove(&key);
    }
}

pub fn is_eligible_user(env: &Env, user: &Address) -> bool {
    env.storage().persistent().has(&DataKey::EligibleUser(user.clone()))
}

pub fn set_eligible_user(env: &Env, user: &Address) {
    let key = DataKey::EligibleUser(user.clone());
    env.storage().persistent().set(&key, &true);
    extend_persistent_ttl(env, &key);
}

pub fn remove_eligible_user(env: &Env, user: &Address) {
    let key = DataKey::EligibleUser(user.clone());
    if env.storage().persistent().has(&key) {
        env.storage().persistent().remove(&key);
    }
}

pub fn extend_instance_ttl(env: &Env) {
    env.storage().instance().extend_ttl(100_000, 200_000);
}

pub fn extend_persistent_ttl(env: &Env, key: &DataKey) {
    env.storage().persistent().extend_ttl(key, 100_000, 200_000);
}
