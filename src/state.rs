use cosmwasm_std::{Uint128, Addr};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use secret_toolkit_storage::{Keymap, Item};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub lp_token_contract: Addr,
    pub lp_token_hash: String,
    pub total_staked: Uint128,
    pub contract_manager: Addr,
    pub reward_tokens: Vec<RewardTokenInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RewardTokenInfo {
    pub reward_token_contract: Addr,
    pub reward_token_hash: String,
    pub reward_per_token_stored: Uint128,
    pub last_updated_time: u64,
    pub reward_streams: Vec<RewardStream>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RewardStream {
    pub total_rewards: Uint128,
    pub release_rate: Uint128,
    pub start_time: u64,
    pub end_time: u64,
}

pub static STATE: Item<State> = Item::new(b"state");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserRewardInfo {
    pub reward_token_contract: Addr,
    pub reward_debt: Uint128,
    pub pending_rewards: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserInfo {
    pub amount_staked: Uint128,
    pub rewards_info: Vec<UserRewardInfo>,
}

pub const USER_INFO: Keymap<Addr, UserInfo> = Keymap::new(b"user_info");









