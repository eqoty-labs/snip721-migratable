use cosmwasm_std::{
    entry_point, to_binary, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult,
};
use snip721_reference_impl::contract::mint;
use snip721_reference_impl::msg::{
    ContractStatus, InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg,
};
use snip721_reference_impl::state::{load, Config, CONFIG_KEY};

use crate::msg::{ExecuteMsg, ExecuteMsgExt, InstantiateMsg, QueryAnswer, QueryMsg, QueryMsgExt};
use crate::state::{config, config_read, State};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    if msg.prices.len() == 0 {
        return Err(StdError::generic_err(format!(
            "No purchase prices were specified"
        )));
    }
    let state = State {
        prices: msg.prices,
        public_metadata: msg.public_metadata,
        private_metadata: msg.private_metadata,
    };

    config(deps.storage).save(&state)?;
    let instantiate_msg = Snip721InstantiateMsg {
        name: "PurchasableSnip721".to_string(),
        symbol: "PUR721".to_string(),
        admin: Some(msg.admin.clone()),
        entropy: msg.entropy,
        royalty_info: msg.royalty_info,
        config: Some(InstantiateConfig {
            public_token_supply: Some(true),
            public_owner: Some(true),
            enable_sealed_metadata: None,
            unwrapped_metadata_is_private: None,
            minter_may_update_metadata: None,
            owner_may_update_metadata: None,
            enable_burn: Some(false),
        }),
        post_init_callback: None,
    };
    snip721_reference_impl::contract::instantiate(&mut deps, env, info.clone(), instantiate_msg)
        .unwrap();

    deps.api
        .debug(format!("PurchasableSnip721 was initialized by {}", info.sender).as_str());
    Ok(Response::default())
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let mut deps = deps;
    return match msg {
        ExecuteMsg::Base(base_msg) => {
            snip721_reference_impl::contract::execute(deps, env, info, base_msg)
        }
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::PurchaseMint { .. } => {
                purchase_and_mint(&mut deps, env, info, &mut config)
            }
        },
    };
}

fn purchase_and_mint(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    config: &mut Config,
) -> StdResult<Response> {
    let state = config_read(deps.storage).load()?;
    if info.funds.len() != 1 {
        return Err(StdError::generic_err(format!(
            "Purchase requires one coin denom to be sent with transaction, {} were sent.",
            info.funds.len()
        )));
    }
    let msg_fund = &info.funds[0];
    let selected_coin_price = state.prices.iter().find(|c| c.denom == msg_fund.denom);
    if let Some(selected_coin_price) = selected_coin_price {
        if msg_fund.amount != selected_coin_price.amount {
            return Err(StdError::generic_err(format!(
                "Purchase price in {} is {}, but {} was sent",
                selected_coin_price.denom, selected_coin_price.amount, msg_fund
            )));
        }
    } else {
        return Err(StdError::generic_err(format!(
            "Purchasing in denom:{} is not allowed",
            msg_fund.denom
        )));
    }
    let sender = info.clone().sender;
    let pay_to_addr = deps.api.addr_humanize(&config.admin).unwrap();
    let send_funds_messages = vec![CosmosMsg::Bank(BankMsg::Send {
        to_address: pay_to_addr.to_string(),
        amount: info.funds.clone(),
    })];
    let admin_addr = deps.api.addr_humanize(&config.admin).unwrap();

    let mint_result = mint(
        deps,
        &env,
        &admin_addr,
        config,
        ContractStatus::Normal.to_u8(),
        None,
        Some(sender.to_string()),
        state.public_metadata,
        state.private_metadata,
        None,
        None,
        None,
        None,
    );
    if let Err(mint_err) = mint_result {
        return Err(mint_err);
    };
    let mint_res = mint_result.unwrap().clone();
    Ok(Response::new()
        .add_messages(send_funds_messages)
        .add_attributes(mint_res.attributes)
        .set_data(mint_res.data.unwrap()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    return match msg {
        QueryMsg::Base(base_msg) => snip721_reference_impl::contract::query(deps, _env, base_msg),
        QueryMsg::Ext(ext_msg) => match ext_msg {
            QueryMsgExt::GetPrices {} => query_prices(deps),
        },
    };
}

/// Returns StdResult<Binary> displaying prices to mint in all acceptable currency denoms
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
pub fn query_prices(deps: Deps) -> StdResult<Binary> {
    let state = config_read(deps.storage).load()?;

    to_binary(&QueryAnswer::GetPrices {
        prices: state.prices,
    })
}