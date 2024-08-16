use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Uint128, Addr};

use crate::state::State;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub lp_token_contract: String,
    pub lp_token_hash: String,
    pub erth_contract: String,
    pub erth_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AllocationPercentage {
    pub address: String,
    pub percentage: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Withdraw {
        amount: Uint128,
    },
    Claim {},
    AddRewardToken {
        contract: String,
        hash: String,
    },
    UpdateRewardTokenHash {
        reward_token_contract: String,
        new_hash: String,
    },
    Receive {
        sender: String,
        from: String,
        amount: Uint128,
        memo: Option<String>,
        msg: Binary,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    Deposit {},
    AddRewards {
        release_duration: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    QueryState {},
    QueryPendingRewards { user: String}
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct StateResponse {
    pub state: State,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PendingRewardsResponse {
    pub rewards: Vec<PendingRewardInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PendingRewardInfo {
    pub reward_token_contract: Addr,
    pub pending_rewards: Uint128,
}