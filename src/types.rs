use soroban_sdk::{contracterror, contracttype, Address, String, Vec};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CouponStatus {
    Created = 0,
    Minted = 1,
    Claimed = 2,
    Redeemed = 3,
    Burned = 4,
    Expired = 5,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum RedemptionStatus {
    Redeemed = 1,
    Expired = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Coupon {
    pub id: u32,
    pub campaign_id: u32,
    pub owner_wallet: Option<Address>,
    pub status: CouponStatus,
    
    // Suporte a encadeamento futuro (inicializados como None)
    pub next_coupons: Option<Vec<u32>>,
    pub previous_coupons: Option<Vec<u32>>,
    pub unlock_conditions: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Campaign {
    pub id: u32,
    pub admin: Address,
    pub max_supply: u32,
    pub current_supply: u32,
    pub expiration_time: u64, // Unix timestamp in segundos
    pub metadata_uri: String,
    pub is_paused: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Campaign,
    Coupon(u32),
    Claimed(Address),
    AuthorizedRedeemer(Address),
    EligibleUser(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    CampaignExpired = 3,
    MaxSupplyReached = 4,
    CouponAlreadyClaimed = 5,
    NotCouponOwner = 6,
    InvalidCouponStatus = 7,
    NotAuthorizedRedeemer = 8,
    CampaignPaused = 9,
    UserNotEligible = 10,
}
