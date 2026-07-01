#![cfg(test)]
use super::*;
use super::types::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String, Vec, vec,
};

fn setup_test(
    env: &Env,
) -> (
    BairrouCouponsContractClient<'static>,
    Address,
    Address,
    Address,
    Vec<Address>,
) {
    let contract_id = env.register_contract(None, BairrouCouponsContract);
    let client = BairrouCouponsContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let user = Address::generate(env);
    let partner = Address::generate(env);
    let authorized_redeemers = vec![env, partner.clone()];

    (client, admin, user, partner, authorized_redeemers)
}

#[test]
fn test_initialize() {
    let env = Env::default();
    let (client, admin, _user, partner, authorized_redeemers) = setup_test(&env);

    let metadata_uri = String::from_str(&env, "ipfs://test");
    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);

    let info = client.get_campaign_info().unwrap();
    assert_eq!(info.id, 1);
    assert_eq!(info.admin, admin);
    assert_eq!(info.max_supply, 100);
    assert_eq!(info.current_supply, 0);
    assert_eq!(info.expiration_time, 10_000);
    assert_eq!(info.metadata_uri, metadata_uri);
    assert_eq!(info.is_paused, false);

    assert!(client.is_redeemer(&partner));

    let res = client.try_initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    assert!(res.is_err());
}

#[test]
fn test_claim_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &10, &10_000, &metadata_uri, &authorized_redeemers);

    client.add_eligible_user(&user);

    let coupon_id = client.claim(&user);
    assert_eq!(coupon_id, 1);

    let info = client.get_campaign_info().unwrap();
    assert_eq!(info.current_supply, 1);

    let coupon = client.get_coupon(&coupon_id).unwrap();
    assert_eq!(coupon.id, 1);
    assert_eq!(coupon.campaign_id, 1);
    assert_eq!(coupon.owner_wallet, Some(user.clone()));
    assert_eq!(coupon.status, CouponStatus::Claimed);

    let coupon_id_2 = client.claim(&user);
    assert_eq!(coupon_id_2, 2);

    let claimed_list = client.get_claimed_coupons(&user);
    assert_eq!(claimed_list.len(), 2);
    assert_eq!(claimed_list.get(0).unwrap(), 1);
    assert_eq!(claimed_list.get(1).unwrap(), 2);
}

#[test]
fn test_claim_max_supply() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &2, &10_000, &metadata_uri, &authorized_redeemers);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    client.add_eligible_user(&user1);
    client.add_eligible_user(&user2);
    client.add_eligible_user(&user3);

    client.claim(&user1);
    client.claim(&user2);

    let res = client.try_claim(&user3);
    assert!(res.is_err());
}

#[test]
fn test_claim_expired() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    env.ledger().set_timestamp(10_001);

    let res = client.try_claim(&user);
    assert!(res.is_err());
}

#[test]
fn test_claim_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    client.set_paused(&true);

    let res = client.try_claim(&user);
    assert!(res.is_err());

    client.set_paused(&false);
    let coupon_id = client.claim(&user);
    assert_eq!(coupon_id, 1);
}

#[test]
fn test_claim_not_eligible() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);

    let res = client.try_claim(&user);
    assert!(res.is_err());

    client.add_eligible_user(&user);
    assert!(client.is_eligible(&user));

    let coupon_id = client.claim(&user);
    assert_eq!(coupon_id, 1);
}

#[test]
fn test_redeem_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    let coupon_id = client.claim(&user);

    let status = client.redeem(&user, &coupon_id, &partner);
    assert_eq!(status, RedemptionStatus::Redeemed);

    let coupon_opt = client.get_coupon(&coupon_id);
    assert!(coupon_opt.is_none());

    let res = client.try_redeem(&user, &coupon_id, &partner);
    assert!(res.is_err());
}

#[test]
fn test_redeem_not_owner() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    let coupon_id = client.claim(&user);

    let other_user = Address::generate(&env);
    
    let res = client.try_redeem(&other_user, &coupon_id, &partner);
    assert!(res.is_err());
}

#[test]
fn test_redeem_not_authorized_partner() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    let coupon_id = client.claim(&user);

    let unauthorized_partner = Address::generate(&env);

    let res = client.try_redeem(&user, &coupon_id, &unauthorized_partner);
    assert!(res.is_err());
}

#[test]
fn test_redeem_expired_burns() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    let coupon_id = client.claim(&user);

    env.ledger().set_timestamp(10_001);

    let status = client.redeem(&user, &coupon_id, &partner);
    assert_eq!(status, RedemptionStatus::Expired);

    let coupon_opt = client.get_coupon(&coupon_id);
    assert!(coupon_opt.is_none());
}

#[test]
fn test_admin_add_remove_redeemer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _user, partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);

    let new_partner = Address::generate(&env);
    assert!(!client.is_redeemer(&new_partner));

    client.add_redeemer(&new_partner);
    assert!(client.is_redeemer(&new_partner));

    client.remove_redeemer(&partner);
    assert!(!client.is_redeemer(&partner));

    client.extend_expiration(&15_000);
    assert_eq!(client.get_campaign_info().unwrap().expiration_time, 15_000);
}

#[test]
fn test_admin_burn() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);
    client.add_eligible_user(&user);

    let coupon_id = client.claim(&user);

    client.burn(&coupon_id);

    let coupon_opt = client.get_coupon(&coupon_id);
    assert!(coupon_opt.is_none());
}

#[test]
fn test_admin_add_remove_eligible_user() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, user, _partner, authorized_redeemers) = setup_test(&env);
    let metadata_uri = String::from_str(&env, "ipfs://test");

    client.initialize(&admin, &1, &100, &10_000, &metadata_uri, &authorized_redeemers);

    assert!(!client.is_eligible(&user));

    client.add_eligible_user(&user);
    assert!(client.is_eligible(&user));

    client.remove_eligible_user(&user);
    assert!(!client.is_eligible(&user));

    let other_user = Address::generate(&env);
    let users = vec![&env, user.clone(), other_user.clone()];
    client.add_eligible_users(&users);
    
    assert!(client.is_eligible(&user));
    assert!(client.is_eligible(&other_user));
}
