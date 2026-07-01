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
    /// Inicializa a campanha de cupons. Pode ser chamada apenas uma vez.
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

        // Registra os estabelecimentos autorizados iniciais
        for redeemer in authorized_redeemers.iter() {
            storage::set_redeemer(&env, &redeemer);
        }

        // Emite evento de inicialização da campanha
        env.events().publish(
            (symbol_short!("init"), campaign_id),
            admin,
        );

        Ok(())
    }

    /// Adiciona um novo estabelecimento parceiro autorizado a resgatar os cupons.
    pub fn add_redeemer(env: Env, partner: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::set_redeemer(&env, &partner);

        Ok(())
    }

    /// Remove a autorização de um estabelecimento parceiro.
    pub fn remove_redeemer(env: Env, partner: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::remove_redeemer(&env, &partner);

        Ok(())
    }

    /// Prorroga a validade da campanha (data de expiração).
    pub fn extend_expiration(env: Env, new_expiration_time: u64) -> Result<(), Error> {
        let mut campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        campaign.expiration_time = new_expiration_time;
        storage::set_campaign(&env, &campaign);

        Ok(())
    }

    /// Ativa ou pausa a campanha promocional de cupons.
    pub fn set_paused(env: Env, paused: bool) -> Result<(), Error> {
        let mut campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        campaign.is_paused = paused;
        storage::set_campaign(&env, &campaign);

        Ok(())
    }

    /// Adiciona um usuário à whitelist de clientes elegíveis.
    pub fn add_eligible_user(env: Env, user: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::set_eligible_user(&env, &user);

        Ok(())
    }

    /// Adiciona vários usuários à whitelist de clientes elegíveis em lote.
    pub fn add_eligible_users(env: Env, users: Vec<Address>) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        for user in users.iter() {
            storage::set_eligible_user(&env, &user);
        }

        Ok(())
    }

    /// Remove um usuário da whitelist de clientes elegíveis.
    pub fn remove_eligible_user(env: Env, user: Address) -> Result<(), Error> {
        let campaign = storage::get_campaign(&env)?;
        campaign.admin.require_auth();

        storage::remove_eligible_user(&env, &user);

        Ok(())
    }

    /// Emissão dinâmica (claim) de um cupom para a carteira do usuário.
    pub fn claim(env: Env, user: Address) -> Result<u32, Error> {
        user.require_auth();

        let mut campaign = storage::get_campaign(&env)?;

        // Validações de segurança
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

        // Incrementa o fornecimento e gera o ID
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

        // Salva o cupom na storage persistente
        storage::set_coupon(&env, coupon_id, &coupon);

        // Registra que o usuário já reivindicou seu cupom
        storage::add_claimed_coupon(&env, &user, coupon_id);

        // Salva o estado atualizado da campanha
        storage::set_campaign(&env, &campaign);

        // Emite o evento on-chain
        env.events().publish(
            (symbol_short!("claim"), user, coupon_id),
            campaign.id,
        );

        Ok(coupon_id)
    }

    /// Efetua o resgate do cupom no balcão do estabelecimento comercial.
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

        // Busca o cupom
        let coupon = match storage::get_coupon(&env, coupon_id) {
            Some(c) => c,
            None => return Err(Error::InvalidCouponStatus),
        };

        // Validação: Proprietário do cupom
        if coupon.owner_wallet != Some(user.clone()) {
            return Err(Error::NotCouponOwner);
        }

        // Validação: Expiração do Cupom
        if env.ledger().timestamp() >= campaign.expiration_time {
            // Emite evento de expiração antes da queima
            env.events().publish(
                (symbol_short!("expired"), user.clone(), coupon_id),
                campaign.id,
            );

            // Destrói e limpa a storage persistente (economiza aluguel/rent)
            storage::remove_coupon(&env, coupon_id);

            // Emite o evento final de queima
            env.events().publish(
                (symbol_short!("burn"), user, coupon_id),
                campaign.id,
            );

            return Ok(RedemptionStatus::Expired);
        }

        // Validação: Estado do cupom (precisa estar CLAIMED)
        if coupon.status != CouponStatus::Claimed {
            return Err(Error::InvalidCouponStatus);
        }

        // Validação: Estabelecimento credenciado
        if !storage::is_redeemer(&env, &redemption_partner) {
            return Err(Error::NotAuthorizedRedeemer);
        }

        // Transição: Redeemed e depois Burned (remoção física da storage)
        env.events().publish(
            (symbol_short!("redeem"), user.clone(), coupon_id, redemption_partner),
            campaign.id,
        );

        // Limpa a storage (Burn)
        storage::remove_coupon(&env, coupon_id);

        env.events().publish(
            (symbol_short!("burn"), user, coupon_id),
            campaign.id,
        );

        Ok(RedemptionStatus::Redeemed)
    }

    /// Destrói permanentemente um cupom ativo por razões administrativas.
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

    /// Retorna as informações detalhadas de um cupom ativo se ele existir na storage.
    pub fn get_coupon(env: Env, coupon_id: u32) -> Option<Coupon> {
        storage::get_coupon(&env, coupon_id)
    }

    /// Retorna as informações gerais da campanha.
    pub fn get_campaign_info(env: Env) -> Option<Campaign> {
        storage::get_campaign(&env).ok()
    }

    /// Retorna se o parceiro está autorizado a fazer resgates.
    pub fn is_redeemer(env: Env, partner: Address) -> bool {
        storage::is_redeemer(&env, &partner)
    }

    /// Retorna a lista de IDs dos cupons reivindicados por um endereço.
    pub fn get_claimed_coupons(env: Env, user: Address) -> Vec<u32> {
        storage::get_claimed_coupons(&env, &user)
    }

    /// Retorna se o usuário está na whitelist de elegibilidade.
    pub fn is_eligible(env: Env, user: Address) -> bool {
        storage::is_eligible_user(&env, &user)
    }
}

#[cfg(test)]
mod test;
