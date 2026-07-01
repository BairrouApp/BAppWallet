#![no_std]

pub mod types;
mod storage;

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, Env, String, Vec,
};
use crate::types::{Campaign, Coupon, CouponStatus, Error, RedemptionStatus};

#[contract]
pub struct BairrouCouponsContract;

#[contractimpl]
impl BairrouCouponsContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        campaign_id: u32,
        max_supply: u32,
        expiration_time: u64,
        metadata_uri: String,
        authorized_redeemers: Vec<Address>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&types::DataKey::Campaign) {
            return Err(Error::AlreadyInitialized);
        }

        let campaign = Campaign {
            id: campaign_id,
            admin: admin.clone(),
            max_supply,
            current_supply: 0,
            expiration_time,
            metadata_uri,
            is_paused: false,
        };

        storage::set_campaign(&env, &campaign);

        for redeemer in authorized_redeemers.iter() {
            storage::set_redeemer(&env, &redeemer);
        }

        env.events().publish(
            (symbol_short!("init"), campaign_id),
            admin,
        );

        Ok(())
    }

    pub fn add_redeemer(env: Env, partner: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::set_redeemer(&env, &partner);

        Ok(())
    }

    pub fn remove_redeemer(env: Env, partner: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::remove_redeemer(&env, &partner);

        Ok(())
    }

    pub fn extend_expiration(env: Env, new_expiration_time: u64) -> Result<(), Error> {
        let mut campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        campaign.expiration_time = new_expiration_time;
        storage::set_campaign(&env, &campaign);

        Ok(())
    }

    pub fn set_paused(env: Env, paused: bool) -> Result<(), Error> {
        let mut campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        campaign.is_paused = paused;
        storage::set_campaign(&env, &campaign);

        Ok(())
    }

    pub fn add_eligible_user(env: Env, user: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::set_eligible_user(&env, &user);

        Ok(())
    }

    pub fn add_eligible_users(env: Env, users: Vec<Address>) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        for user in users.iter() {
            storage::set_eligible_user(&env, &user);
        }

        Ok(())
    }

    pub fn remove_eligible_user(env: Env, user: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::remove_eligible_user(&env, &user);

        Ok(())
    }

    pub fn claim(env: Env, user: Address) -> Result<u32, Error> {
        user.require_auth();

        let mut campaign = storage::get_campaign(&env)?;

        if !storage::is_eligible_user(&env, &user) {
            return Err(Error::UserNotEligible);
        }

        if campaign.is_paused {
            return Err(Error::CampaignPaused);
        }

        if env.ledger().timestamp() >= campaign.expiration_time {
            return Err(Error::CampaignExpired);
        }

        if campaign.current_supply >= campaign.max_supply {
            return Err(Error::MaxSupplyReached);
        }

        campaign.current_supply += 1;
        let coupon_id = campaign.current_supply;

        let coupon = Coupon {
            id: coupon_id,
            campaign_id: campaign.id,
            owner_wallet: Some(user.clone()),
            status: CouponStatus::Claimed,
            next_coupons: None,
            previous_coupons: None,
            unlock_conditions: None,
        };

        storage::set_coupon(&env, coupon_id, &coupon);

        storage::add_claimed_coupon(&env, &user, coupon_id);

        storage::set_campaign(&env, &campaign);

        env.events().publish(
            (symbol_short!("claim"), user, coupon_id),
            campaign.id,
        );

        Ok(coupon_id)
    }

    pub fn redeem(
        env: Env,
        user: Address,
        coupon_id: u32,
        redemption_partner: Address,
    ) -> Result<RedemptionStatus, Error> {
        redemption_partner.require_auth();

        let campaign = storage::get_campaign(&env)?;

        if campaign.is_paused {
            return Err(Error::CampaignPaused);
        }

        let coupon = match storage::get_coupon(&env, coupon_id) {
            Some(c) => c,
            None => return Err(Error::InvalidCouponStatus),
        };

        if coupon.owner_wallet != Some(user.clone()) {
            return Err(Error::NotCouponOwner);
        }

        if env.ledger().timestamp() >= campaign.expiration_time {
            env.events().publish(
                (symbol_short!("expired"), user.clone(), coupon_id),
                campaign.id,
            );

            storage::remove_coupon(&env, coupon_id);

            env.events().publish(
                (symbol_short!("burn"), user, coupon_id),
                campaign.id,
            );

            return Ok(RedemptionStatus::Expired);
        }

        if coupon.status != CouponStatus::Claimed {
            return Err(Error::InvalidCouponStatus);
        }

        if !storage::is_redeemer(&env, &redemption_partner) {
            return Err(Error::NotAuthorizedRedeemer);
        }

        env.events().publish(
            (symbol_short!("redeem"), user.clone(), coupon_id, redemption_partner),
            campaign.id,
        );

        storage::remove_coupon(&env, coupon_id);

        env.events().publish(
            (symbol_short!("burn"), user, coupon_id),
            campaign.id,
        );

        Ok(RedemptionStatus::Redeemed)
    }

    pub fn burn(env: Env, coupon_id: u32) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        if let Some(coupon) = storage::get_coupon(&env, coupon_id) {
            let owner = coupon.owner_wallet.unwrap_or(campaign.admin.clone());

            storage::remove_coupon(&env, coupon_id);

            env.events().publish(
                (symbol_short!("burn"), owner, coupon_id),
                campaign.id,
            );
            Ok(())
        } else {
            Err(Error::InvalidCouponStatus)
        }
    }

    pub fn get_coupon(env: Env, coupon_id: u32) -> Option<Coupon> {
        storage::get_coupon(&env, coupon_id)
    }

    pub fn get_campaign_info(env: Env) -> Option<Campaign> {
        storage::get_campaign(&env).ok()
    }

    pub fn is_redeemer(env: Env, partner: Address) -> bool {
        storage::is_redeemer(&env, &partner)
    }

    pub fn get_claimed_coupons(env: Env, user: Address) -> Vec<u32> {
        storage::get_claimed_coupons(&env, &user)
    }

    pub fn is_eligible(env: Env, user: Address) -> bool {
        storage::is_eligible_user(&env, &user)
    }
}

#[cfg(test)]
mod test;
