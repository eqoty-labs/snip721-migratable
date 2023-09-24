use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, ContractInfo, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, StdResult, SubMsg, WasmMsg,
};
use cw_migratable_contract_std::execute::{
    add_migration_complete_event_subscriber, register_to_notify_on_migration_complete,
    update_migrated_subscriber,
};
use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
use cw_migratable_contract_std::state::canonicalize;
use snip721_reference_impl::msg::ExecuteMsg::{ChangeAdmin, MintNft};
use snip721_reference_impl::msg::{InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg};

use crate::msg::{
    DealerExecuteMsg, DealerQueryMsg, ExecuteMsg, InstantiateMsg, QueryAnswer, QueryMsg,
};
use crate::msg_external::MigratableSnip721InstantiateMsg;
use crate::state::{
    PurchasableMetadata, ADMIN, CHILD_SNIP721_ADDRESS, CHILD_SNIP721_CODE_HASH,
    PURCHASABLE_METADATA, PURCHASE_PRICES,
};

const INSTANTIATE_SNIP721_REPLY_ID: u64 = 1u64;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    if msg.prices.is_empty() {
        return Err(StdError::generic_err("No purchase prices were specified"));
    }
    // instantiate the child snip721 w/ this contract as admin to add this contract to its list of
    // minters. Then set a second msg in Reply to change the admin to true_admin
    let temp_snip721_admin = env.contract.address;
    let true_admin = match msg.admin {
        Some(admin) => deps.api.addr_validate(admin.as_str())?,
        None => info.sender,
    };
    ADMIN.save(
        deps.storage,
        &deps.api.addr_canonicalize(true_admin.as_str())?,
    )?;
    PURCHASE_PRICES.save(deps.storage, &msg.prices)?;
    CHILD_SNIP721_CODE_HASH.save(deps.storage, &msg.snip721_code_hash)?;
    PURCHASABLE_METADATA.save(
        deps.storage,
        &PurchasableMetadata {
            public_metadata: msg.public_metadata,
            private_metadata: msg.private_metadata,
        },
    )?;
    let instantiate_msg = MigratableSnip721InstantiateMsg {
        instantiate: Snip721InstantiateMsg {
            name: "PurchasableSnip721".to_string(),
            symbol: "PUR721".to_string(),
            admin: Some(temp_snip721_admin.to_string()),
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
            post_init_data: None,
        },
        max_migration_complete_event_subscribers: 1,
    };
    let instantiate_wasm_msg = WasmMsg::Instantiate {
        code_id: msg.snip721_code_id,
        code_hash: msg.snip721_code_hash,
        msg: to_binary(&instantiate_msg).unwrap(),
        funds: vec![],
        label: msg.snip721_label,
    };

    Ok(Response::new().add_submessages([SubMsg::reply_on_success(
        instantiate_wasm_msg,
        INSTANTIATE_SNIP721_REPLY_ID,
    )]))
}

#[entry_point]
pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut deps = deps;
    match msg {
        ExecuteMsg::Dealer(dealer_msg) => match dealer_msg {
            DealerExecuteMsg::PurchaseMint { .. } => purchase_and_mint(&mut deps, info),
        },
        ExecuteMsg::Migrate(migrate_msg) => match migrate_msg {
            MigratableExecuteMsg::SubscribeToMigrationCompleteEvent { address, code_hash } => {
                register_to_notify_on_migration_complete(deps, address, code_hash)
            }
            _ => Err(StdError::generic_err("Unsupported Migrate message")),
        },
        ExecuteMsg::MigrateListener(migrate_listener_msg) => match migrate_listener_msg {
            MigrationListenerExecuteMsg::MigrationCompleteNotification { to, .. } => {
                update_child_snip721(deps, info, to)
            }
        },
    }
}

fn update_child_snip721(
    deps: DepsMut,
    info: MessageInfo,
    migrated_to: ContractInfo,
) -> StdResult<Response> {
    let current_child_snip721_address = CHILD_SNIP721_ADDRESS.load(deps.storage)?;
    let raw_sender = deps.api.addr_canonicalize(info.sender.as_str())?;
    if raw_sender != current_child_snip721_address {
        return Err(StdError::generic_err(
            "Only the migrated child snip721 is allowed to trigger an update",
        ));
    }
    let raw_migrated_to = canonicalize(deps.api, &migrated_to)?;
    CHILD_SNIP721_ADDRESS.save(deps.storage, &raw_migrated_to.address)?;
    CHILD_SNIP721_CODE_HASH.save(deps.storage, &raw_migrated_to.code_hash)?;

    update_migrated_subscriber(deps.storage, &raw_sender, &raw_migrated_to)?;
    Ok(Response::new())
}

fn purchase_and_mint(deps: &mut DepsMut, info: MessageInfo) -> StdResult<Response> {
    if info.funds.len() != 1 {
        return Err(StdError::generic_err(format!(
            "Purchase requires one coin denom to be sent with transaction, {} were sent.",
            info.funds.len()
        )));
    }
    let msg_fund = &info.funds[0];
    let prices = PURCHASE_PRICES.load(deps.storage)?;
    let selected_coin_price = prices.iter().find(|c| c.denom == msg_fund.denom);
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
    let admin_addr = &deps.api.addr_humanize(&ADMIN.load(deps.storage)?)?;
    let send_funds_bank_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: admin_addr.to_string(),
        amount: info.funds.clone(),
    });
    let purchasable_metadata: PurchasableMetadata = PURCHASABLE_METADATA.load(deps.storage)?;
    let mint_nft_msg = MintNft {
        token_id: None,
        owner: Some(sender.to_string()),
        public_metadata: purchasable_metadata.public_metadata,
        private_metadata: purchasable_metadata.private_metadata,
        serial_number: None,
        royalty_info: None,
        transferable: None,
        memo: None,
        padding: None,
    };
    let child_snip721_code_hash = CHILD_SNIP721_CODE_HASH.load(deps.storage)?;
    let child_snip721_address = CHILD_SNIP721_ADDRESS.load(deps.storage)?;
    let mint_wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&child_snip721_address)?.to_string(),
        code_hash: child_snip721_code_hash,
        msg: to_binary(&mint_nft_msg)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessages([SubMsg::new(send_funds_bank_msg), SubMsg::new(mint_wasm_msg)]))
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        INSTANTIATE_SNIP721_REPLY_ID => on_instantiated_snip721_reply(deps, env, msg),
        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

fn on_instantiated_snip721_reply(deps: DepsMut, env: Env, reply: Reply) -> StdResult<Response> {
    let result = reply.result.unwrap();
    let contract_address = &result
        .events
        .iter()
        .find(|e| e.ty == "instantiate")
        .unwrap()
        .attributes
        .iter()
        .find(|a| a.key == "contract_address")
        .unwrap()
        .value;
    let child_snip721_address = deps.api.addr_validate(contract_address.as_str())?;
    let raw_child_snip721_address = deps.api.addr_canonicalize(child_snip721_address.as_str())?;
    CHILD_SNIP721_ADDRESS.save(deps.storage, &raw_child_snip721_address)?;
    let child_snip721_code_hash: String = CHILD_SNIP721_CODE_HASH.load(deps.storage)?;
    let admin: Addr = deps.api.addr_humanize(&ADMIN.load(deps.storage)?)?;
    add_migration_complete_event_subscriber(
        deps.storage,
        &raw_child_snip721_address,
        &child_snip721_code_hash,
    )?;

    let subscribe_to_migration_complete_event_wasm_msg = WasmMsg::Execute {
        contract_addr: child_snip721_address.to_string(),
        code_hash: child_snip721_code_hash.clone(),
        msg: to_binary(&MigratableExecuteMsg::SubscribeToMigrationCompleteEvent {
            address: env.contract.address.to_string(),
            code_hash: env.contract.code_hash,
        })?,
        funds: vec![],
    };

    let change_admin_to_true_admin_wasm_msg = WasmMsg::Execute {
        contract_addr: child_snip721_address.to_string(),
        code_hash: child_snip721_code_hash,
        msg: to_binary(&ChangeAdmin {
            address: admin.to_string(),
            padding: None,
        })
        .unwrap(),
        funds: vec![],
    };

    Ok(Response::new().add_submessages([
        SubMsg::new(subscribe_to_migration_complete_event_wasm_msg),
        SubMsg::new(change_admin_to_true_admin_wasm_msg),
    ]))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Dealer(dealer_msg) => match dealer_msg {
            DealerQueryMsg::GetPrices {} => query_prices(deps),
            DealerQueryMsg::GetChildSnip721 {} => query_child_snip721(deps),
        },
    }
}

fn query_child_snip721(deps: Deps) -> StdResult<Binary> {
    to_binary(&QueryAnswer::ContractInfo(ContractInfo {
        address: deps
            .api
            .addr_humanize(&CHILD_SNIP721_ADDRESS.load(deps.storage)?)?,
        code_hash: CHILD_SNIP721_CODE_HASH.load(deps.storage)?,
    }))
}

/// Returns StdResult<Binary> displaying prices to mint in all acceptable currency denoms
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
pub fn query_prices(deps: Deps) -> StdResult<Binary> {
    to_binary(&QueryAnswer::GetPrices {
        prices: PURCHASE_PRICES.load(deps.storage)?,
    })
}
