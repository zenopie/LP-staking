use cosmwasm_std::{
    entry_point, to_binary, from_binary, Binary, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Addr, Uint128, CosmosMsg,
    WasmMsg,
};
use secret_toolkit::snip20;

use crate::msg::{ExecuteMsg, QueryMsg, StateResponse, ReceiveMsg, InstantiateMsg, PendingRewardsResponse,
    PendingRewardInfo,
};
use crate::state::{STATE, State, RewardTokenInfo, RewardStream, UserRewardInfo, UserInfo, USER_INFO};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let lp_token_contract = deps.api.addr_validate(&msg.lp_token_contract)?;
    let erth_token_contract = deps.api.addr_validate(&msg.erth_contract)?;

    let erth_token_info = RewardTokenInfo {
        reward_token_contract: erth_token_contract,
        reward_token_hash: msg.erth_hash.clone(),
        reward_per_token_stored: Uint128::zero(),
        last_updated_time: env.block.time.seconds(),
        reward_streams: vec![],
    };

    let state = State {
        lp_token_contract: lp_token_contract.clone(),
        lp_token_hash: msg.lp_token_hash,
        total_staked: Uint128::zero(),
        contract_manager: info.sender,
        reward_tokens: vec![erth_token_info],
    };

    // Register this contract as a receiver for the LP token
    let register_lp_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token_contract.to_string(),
        code_hash: state.lp_token_hash.clone(),
        msg: to_binary(&snip20::HandleMsg::RegisterReceive {
            code_hash: env.contract.code_hash.clone(),
            padding: None,  // Optional padding
        })?,
        funds: vec![],
    });

    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_message(register_lp_msg)
        .add_attribute("action", "instantiate")
        .add_attribute("lp_token_contract", msg.lp_token_contract)
        .add_attribute("erth_token_contract", msg.erth_contract)
        .add_attribute("erth_token_hash", msg.erth_hash))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::Withdraw { amount } => execute_withdraw(deps, env, info, amount),
        ExecuteMsg::AddRewardToken { contract, hash } => execute_add_reward_token(deps, env, info, contract, hash),
        ExecuteMsg::Claim {} => execute_claim_rewards(deps, env, info),
        ExecuteMsg::UpdateRewardTokenHash { reward_token_contract, new_hash } => 
            execute_update_reward_token_hash(deps, info, reward_token_contract, new_hash),
        ExecuteMsg::Receive {
            sender,
            from,
            amount,
            msg,
            memo: _,
        } => execute_receive(deps, env, info, sender, from, amount, msg),
    }
}

pub fn execute_add_reward_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    reward_token_contract: String,
    reward_token_hash: String,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;

    if info.sender != state.contract_manager {
        return Err(StdError::generic_err("Not Authorized"));
    }

    let reward_token_addr = deps.api.addr_validate(&reward_token_contract)?;

    if state.reward_tokens.iter().any(|token| token.reward_token_contract == reward_token_addr) {
        return Err(StdError::generic_err("Reward token already added"));
    }

    let reward_token_info = RewardTokenInfo {
        reward_token_contract: reward_token_addr.clone(),
        reward_token_hash: reward_token_hash.clone(),
        reward_per_token_stored: Uint128::zero(),
        last_updated_time: env.block.time.seconds(),
        reward_streams: vec![],  // No streams initially; they are added later
    };

    // Register this contract as a receiver for the reward token
    let register_reward_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: reward_token_addr.to_string(),
        code_hash: reward_token_hash.clone(),
        msg: to_binary(&snip20::HandleMsg::RegisterReceive {
            code_hash: env.contract.code_hash.clone(),
            padding: None,  // Optional padding
        })?,
        funds: vec![],
    });

    state.reward_tokens.push(reward_token_info);
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_message(register_reward_msg)
        .add_attribute("action", "add_reward_token")
        .add_attribute("reward_token_contract", reward_token_contract)
        .add_attribute("reward_token_hash", reward_token_hash))
}

fn execute_update_reward_token_hash(
    deps: DepsMut,
    info: MessageInfo,
    reward_token_contract: String,
    new_hash: String,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;

    if info.sender != state.contract_manager {
        return Err(StdError::generic_err("Not Authorized"));
    }

    let reward_token_addr = deps.api.addr_validate(&reward_token_contract)?;

    let reward_token = state.reward_tokens.iter_mut().find(|token| token.reward_token_contract == reward_token_addr);

    match reward_token {
        Some(token) => {
            token.reward_token_hash = new_hash;
            STATE.save(deps.storage, &state)?;
            Ok(Response::new().add_attribute("method", "update_reward_token_hash"))
        },
        None => Err(StdError::generic_err("Reward token not found")),
    }
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USER_INFO
        .get(deps.storage, &info.sender)
        .ok_or_else(|| StdError::generic_err("No deposit found for this user"))?;

    if user_info.amount_staked < amount {
        return Err(StdError::generic_err("Insufficient staked amount"));
    }

    // First, update rewards and claim pending rewards
    update_rewards(&mut state, &mut user_info, env.block.time.seconds())?;
    let claim_messages = claim_rewards_internal(&info.sender, &mut user_info, &state)?;

    // Now reduce the user's staked amount
    user_info.amount_staked -= amount;
    state.total_staked -= amount;

    // Save the updated state and user info
    STATE.save(deps.storage, &state)?;
    USER_INFO.insert(deps.storage, &info.sender, &user_info)?;

    // Combine the claim messages with the withdraw response
    Ok(Response::new()
        .add_messages(claim_messages)
        .add_attribute("action", "withdraw")
        .add_attribute("amount", amount.to_string()))
}



pub fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USER_INFO
        .get(deps.storage, &info.sender)
        .ok_or_else(|| StdError::generic_err("No deposit found for this user"))?;

    if user_info.amount_staked.is_zero() {
        return Err(StdError::generic_err("Cannot claim rewards without a deposit"));
    }

    // First, update rewards and then claim them
    update_rewards(&mut state, &mut user_info, env.block.time.seconds())?;
    let claim_messages = claim_rewards_internal(&info.sender, &mut user_info, &state)?;

    // Save the updated state and user info
    STATE.save(deps.storage, &state)?;
    USER_INFO.insert(deps.storage, &info.sender, &user_info)?;

    Ok(Response::new()
        .add_messages(claim_messages)
        .add_attribute("action", "claim_rewards"))
}



fn claim_rewards_internal(
    user_addr: &Addr,
    user_info: &mut UserInfo,
    state: &State,
) -> StdResult<Vec<CosmosMsg>> {
    let mut messages = vec![];

    for reward_info in &mut user_info.rewards_info {
        if reward_info.pending_rewards > Uint128::zero() {
            let reward_token_info = state.reward_tokens.iter().find(|token| token.reward_token_contract == reward_info.reward_token_contract)
                .ok_or_else(|| StdError::generic_err("Reward token contract not found in state reward tokens"))?;

            let reward_transfer_msg = snip20::HandleMsg::Transfer {
                recipient: user_addr.to_string(),
                amount: reward_info.pending_rewards,
                memo: None,
                padding: None,
            };

            let wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: reward_token_info.reward_token_contract.to_string(),
                code_hash: reward_token_info.reward_token_hash.clone(),
                msg: to_binary(&reward_transfer_msg)?,
                funds: vec![],
            });

            messages.push(wasm_msg);
            reward_info.pending_rewards = Uint128::zero();
        }
    }

    Ok(messages)
}



pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    from: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, StdError> {
    let msg: ReceiveMsg = from_binary(&msg)?;

    // Validate the `from` and `sender` addresses
    let from_addr = deps.api.addr_validate(&from)?;
    let _sender_addr = deps.api.addr_validate(&sender)?;

    match msg {
        ReceiveMsg::Deposit {} => receive_deposit(deps, env, info, from_addr, amount),
        ReceiveMsg::AddRewards { release_duration } => receive_add_rewards(deps, env, info, amount, release_duration),
    }
}

pub fn receive_deposit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    from: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;

    if info.sender != state.lp_token_contract {
        return Err(StdError::generic_err("Unauthorized token"));
    }

    let mut user_info = USER_INFO
        .get(deps.storage, &from)
        .unwrap_or_default();

    update_rewards(&mut state, &mut user_info, _env.block.time.seconds())?;

    user_info.amount_staked += amount;
    state.total_staked += amount;

    STATE.save(deps.storage, &state)?;
    USER_INFO.insert(deps.storage, &from, &user_info)?;

    Ok(Response::new()
        .add_attribute("action", "deposit")
        .add_attribute("amount", amount.to_string())
        .add_attribute("from", from.to_string()))
}

pub fn receive_add_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    release_duration: u64, // Duration over which the new tokens should be released
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;
    let current_time = env.block.time.seconds();

    if state.total_staked.is_zero() {
        return Err(StdError::generic_err("No staked tokens to distribute rewards"));
    }

    let reward_token_info = state
        .reward_tokens
        .iter_mut()
        .find(|token| token.reward_token_contract == info.sender)
        .ok_or_else(|| StdError::generic_err("Reward token not found"))?;

    // Create a new reward stream with the specified release rate
    let release_rate = amount / Uint128::from(release_duration as u128);
    let end_time = current_time + release_duration;

    let new_stream = RewardStream {
        total_rewards: amount,
        release_rate,
        start_time: current_time,
        end_time,
    };

    reward_token_info.reward_streams.push(new_stream);

    // Update the last updated time to the current block time
    reward_token_info.last_updated_time = current_time;

    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "deposit_rewards")
        .add_attribute("amount", amount.to_string())
        .add_attribute("token_contract", info.sender.clone())
        .add_attribute("release_duration", release_duration.to_string()))
}

fn update_rewards(
    state: &mut State,
    user_info: &mut UserInfo,
    current_time: u64,
) -> StdResult<()> {
    if state.total_staked.is_zero() {
        return Ok(());
    }

    for token in &mut state.reward_tokens {
        let mut rewards_released = Uint128::zero();

        // Use retain to filter out and remove expired streams after processing
        token.reward_streams.retain(|stream| {
            // Calculate the rewards released by this stream based on the time elapsed since it started
            let time_elapsed = current_time.min(stream.end_time) - stream.start_time;
            let stream_rewards = stream.release_rate * Uint128::from(time_elapsed as u128);
            rewards_released += stream_rewards;

            // Determine if the stream should be removed: keep it only if it's still active
            current_time < stream.end_time
        });

        // Update the reward per token stored with the newly released rewards
        token.reward_per_token_stored += rewards_released / state.total_staked;

        // Update the last updated time to the current time
        token.last_updated_time = current_time;

        let user_reward_info = user_info
            .rewards_info
            .iter_mut()
            .find(|info| info.reward_token_contract == token.reward_token_contract);

        let user_reward_info = match user_reward_info {
            Some(info) => info,
            None => {
                user_info.rewards_info.push(UserRewardInfo {
                    reward_token_contract: token.reward_token_contract.clone(),
                    reward_debt: user_info.amount_staked * token.reward_per_token_stored,
                    pending_rewards: Uint128::zero(),
                });
                user_info.rewards_info.last_mut().unwrap()
            }
        };

        let pending_reward = user_info.amount_staked * token.reward_per_token_stored - user_reward_info.reward_debt;
        user_reward_info.pending_rewards += pending_reward;
        user_reward_info.reward_debt = user_info.amount_staked * token.reward_per_token_stored;
    }

    Ok(())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryState {} => to_binary(&query_state(deps)?),
        QueryMsg::QueryPendingRewards { user } => 
            to_binary(&query_pending_rewards(deps, deps.api.addr_validate(&user)?)?),
    }
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse { state: state })
}




fn query_pending_rewards(deps: Deps, user: Addr) -> StdResult<PendingRewardsResponse> {
    let state = STATE.load(deps.storage)?;
    let user_info = USER_INFO
        .get(deps.storage, &user)
        .ok_or_else(|| StdError::generic_err("No deposit found for this user"))?;

    let mut pending_rewards = Vec::new();

    for reward_info in &user_info.rewards_info {
        let reward_token_info = state.reward_tokens.iter()
            .find(|token| token.reward_token_contract == reward_info.reward_token_contract)
            .ok_or_else(|| StdError::generic_err("Reward token contract not found in state reward tokens"))?;

        let pending_reward = reward_info.pending_rewards
            + (user_info.amount_staked * reward_token_info.reward_per_token_stored - reward_info.reward_debt);

        pending_rewards.push(PendingRewardInfo {
            reward_token_contract: reward_info.reward_token_contract.clone(),
            pending_rewards: pending_reward,
        });
    }

    Ok(PendingRewardsResponse { rewards: pending_rewards })
}
