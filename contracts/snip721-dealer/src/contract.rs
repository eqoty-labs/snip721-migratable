use cosmwasm_std::{Addr, BankMsg, Binary, CanonicalAddr, Coin, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, to_binary, WasmMsg};
use schemars::_serde_json::to_string;
use snip721_reference_impl::msg::{InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg};
use snip721_reference_impl::msg::ExecuteMsg::{ChangeAdmin, MintNft};
use snip721_reference_impl::state::{load, save};

use migration::execute::register_on_migration_complete_notify_receiver;
use migration::msg_types::{ContractInfo, MigrateTo};
use migration::msg_types::ReplyError::StateChangesNotAllowed;
use migration::state::{ContractMode, MIGRATED_TO_KEY, MigratedTo};
use snip721_migratable::msg::{ExecuteMsg as Snip721MigratableExecuteMsg, ExecuteMsgExt};

use crate::contract_migrate::{instantiate_with_migrated_config, migrate, query_migrated_info};
use crate::msg::{ExecuteMsg, InstantiateMsg, InstantiateSelfAndChildSnip721Msg, QueryAnswer, QueryMsg};
use crate::msg_external::MigratableSnip721InstantiateMsg;
use crate::state::{ADMIN_KEY, CHILD_SNIP721_ADDRESS_KEY, CHILD_SNIP721_CODE_HASH_KEY, CONTRACT_MODE_KEY, PURCHASABLE_METADATA_KEY, PurchasableMetadata, PURCHASE_PRICES_KEY};

const INSTANTIATE_SNIP721_REPLY_ID: u64 = 1u64;
const MIGRATE_REPLY_ID: u64 = 2u64;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    return match msg {
        InstantiateMsg::New(init) => init_snip721(&mut deps, env, info, init),
        InstantiateMsg::Migrate(init) => {
            let migrate_from = init.migrate_from;
            let migrate_msg = ExecuteMsg::Migrate {
                admin_permit: migrate_from.admin_permit,
                migrate_to: MigrateTo {
                    address: env.contract.address.clone(),
                    code_hash: env.contract.code_hash,
                    entropy: init.entropy,
                },
            };
            let migrate_wasm_msg: WasmMsg = WasmMsg::Execute {
                contract_addr: migrate_from.address.to_string(),
                code_hash: migrate_from.code_hash,
                msg: to_binary(&migrate_msg)?,
                funds: vec![],
            };

            Ok(Response::new()
                .add_submessages([
                    SubMsg::reply_on_success(
                        migrate_wasm_msg,
                        MIGRATE_REPLY_ID,
                    ),
                ])
            )
        }
    };
}

fn init_snip721(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateSelfAndChildSnip721Msg,
) -> StdResult<Response> {
    if msg.prices.len() == 0 {
        return Err(StdError::generic_err(format!(
            "No purchase prices were specified"
        )));
    }
    // instantiate the child snip721 w/ this contract as admin to add this contract to its list of
    // minters. Then set a second msg in Reply to change the admin to true_admin
    let temp_snip721_admin = env.contract.address;
    let true_admin = match msg.admin {
        Some(admin) => deps.api.addr_validate(admin.as_str())?,
        None => info.sender
    };
    save(deps.storage, ADMIN_KEY, &deps.api.addr_canonicalize(true_admin.as_str())?)?;
    save(deps.storage, PURCHASE_PRICES_KEY, &msg.prices)?;
    save(deps.storage, CHILD_SNIP721_CODE_HASH_KEY, &msg.snip721_code_hash)?;
    save(deps.storage, PURCHASABLE_METADATA_KEY,
         &PurchasableMetadata {
             public_metadata: msg.public_metadata,
             private_metadata: msg.private_metadata,
         })?;
    save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::Running)?;
    let instantiate_msg = MigratableSnip721InstantiateMsg::New(Snip721InstantiateMsg {
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
    });
    let instantiate_wasm_msg = WasmMsg::Instantiate {
        code_id: msg.snip721_code_id,
        code_hash: msg.snip721_code_hash,
        msg: to_binary(&instantiate_msg).unwrap(),
        funds: vec![],
        label: msg.snip721_label,
    };

    return Ok(
        Response::new().add_submessages([
            SubMsg::reply_on_success(
                instantiate_wasm_msg,
                INSTANTIATE_SNIP721_REPLY_ID,
            ),
        ])
    );
}


#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut deps = deps;
    let mode = load(deps.storage, CONTRACT_MODE_KEY)?;
    return match mode {
        ContractMode::MigrateDataIn => {
            Err(StdError::generic_err(format!("Illegal Contact Mode: {:?}. This shouldn't happen", mode)))
        }
        ContractMode::Running => {
            match msg {
                ExecuteMsg::PurchaseMint { .. } => {
                    purchase_and_mint(&mut deps, info)
                }
                ExecuteMsg::Migrate { admin_permit, migrate_to } =>
                    migrate(deps, env, info, admin_permit, migrate_to),
                ExecuteMsg::RegisterOnMigrationCompleteNotifyReceiver { address, code_hash } => {
                    let admin = load(deps.storage, ADMIN_KEY)?;
                    register_on_migration_complete_notify_receiver(deps, info, admin, address, code_hash)
                }
                ExecuteMsg::OnMigrationComplete {} =>
                    update_child_snip721(deps, info)
            }
        }
        ContractMode::MigratedOut => {
            let migrated_to: MigratedTo = load(deps.storage, MIGRATED_TO_KEY)?;
            Err(StdError::generic_err(
                to_string(&StateChangesNotAllowed {
                    message: "This contract has been migrated. No further state changes are allowed!".to_string(),
                    migrated_to: migrated_to.contract.into(),
                }).unwrap()
            ))
        }
    };
}

fn update_child_snip721(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let current_child_snip721_address: CanonicalAddr = load(deps.storage, CHILD_SNIP721_ADDRESS_KEY)?;
    let current_child_snip721_code_hash: String = load(deps.storage, CHILD_SNIP721_CODE_HASH_KEY)?;
    let child_snip721_migrated_to: QueryAnswer = deps.querier.query_wasm_smart(
        current_child_snip721_code_hash.clone(),
        deps.api.addr_humanize(&current_child_snip721_address)?,
        &QueryMsg::MigratedTo {},
    ).unwrap();
    if let QueryAnswer::MigrationInfo(Some(migrated_to)) = child_snip721_migrated_to {
        if info.sender != migrated_to.address {
            // The newly migrated contract only calls this after all contracts have been migrated.
            // This prevents someone from updating the child contract before all tokens have been migrated
            return Err(StdError::generic_err(
                "Only the migrated child snip721 is allowed to trigger an update",
            ));
        }
        save(deps.storage, CHILD_SNIP721_ADDRESS_KEY, &deps.api.addr_canonicalize(migrated_to.address.as_str())?)?;
        save(deps.storage, CHILD_SNIP721_CODE_HASH_KEY, &migrated_to.code_hash)?;
        Ok(Response::new())
    } else {
        Err(StdError::generic_err("Child snip721 has not been migrated"))
    }
}

fn purchase_and_mint(
    deps: &mut DepsMut,
    info: MessageInfo,
) -> StdResult<Response> {
    if info.funds.len() != 1 {
        return Err(StdError::generic_err(format!(
            "Purchase requires one coin denom to be sent with transaction, {} were sent.",
            info.funds.len()
        )));
    }
    let msg_fund = &info.funds[0];
    let prices: Vec<Coin> = load(deps.storage, PURCHASE_PRICES_KEY)?;
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
    let admin_addr = &deps.api.addr_humanize(&load::<CanonicalAddr>(deps.storage, ADMIN_KEY)?)?;
    let send_funds_bank_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: admin_addr.to_string(),
        amount: info.funds.clone(),
    });
    let purchasable_metadata: PurchasableMetadata = load(deps.storage, PURCHASABLE_METADATA_KEY)?;
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
    let child_snip721_code_hash: String = load(deps.storage, CHILD_SNIP721_CODE_HASH_KEY)?;
    let child_snip721_address: CanonicalAddr = load(deps.storage, CHILD_SNIP721_ADDRESS_KEY)?;
    let mint_wasm_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&child_snip721_address)?.to_string(),
        code_hash: child_snip721_code_hash,
        msg: to_binary(&mint_nft_msg)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessages([
            SubMsg::new(send_funds_bank_msg),
            SubMsg::new(mint_wasm_msg)
        ])
    )
}


#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        INSTANTIATE_SNIP721_REPLY_ID => on_instantiated_snip721_reply(deps, env, msg),
        MIGRATE_REPLY_ID => instantiate_with_migrated_config(deps, msg),
        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

fn on_instantiated_snip721_reply(deps: DepsMut, env: Env, reply: Reply) -> StdResult<Response> {
    let result = reply.result.unwrap();
    let contract_address = &result.events.iter()
        .find(|e| e.ty == "instantiate").unwrap()
        .attributes.iter()
        .find(|a| a.key == "contract_address").unwrap()
        .value;
    let child_snip721_address = deps.api.addr_validate(contract_address.as_str())?;
    save(deps.storage, CHILD_SNIP721_ADDRESS_KEY, &deps.api.addr_canonicalize(child_snip721_address.as_str())?)?;
    let child_snip721_code_hash: String = load(deps.storage, CHILD_SNIP721_CODE_HASH_KEY)?;
    let admin: Addr = deps.api.addr_humanize(&load::<CanonicalAddr>(deps.storage, ADMIN_KEY)?)?;

    let reg_on_migration_complete_notify_receiver_wasm_msg = WasmMsg::Execute {
        contract_addr: child_snip721_address.to_string(),
        code_hash: child_snip721_code_hash.clone(),
        msg: to_binary(
            &Snip721MigratableExecuteMsg::Ext(
                ExecuteMsgExt::RegisterOnMigrationCompleteNotifyReceiver {
                    address: env.contract.address.to_string(),
                    code_hash: env.contract.code_hash,
                }
            ))?,
        funds: vec![],
    };

    let change_admin_to_true_admin_wasm_msg = WasmMsg::Execute {
        contract_addr: child_snip721_address.to_string(),
        code_hash: child_snip721_code_hash,
        msg: to_binary(&ChangeAdmin { address: admin.to_string(), padding: None }).unwrap(),
        funds: vec![],
    };

    Ok(Response::new()
        .add_submessages([
            SubMsg::new(reg_on_migration_complete_notify_receiver_wasm_msg),
            SubMsg::new(change_admin_to_true_admin_wasm_msg),
        ])
    )
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let mode = load(deps.storage, CONTRACT_MODE_KEY)?;
    return match mode {
        ContractMode::MigratedOut => {
            let migrated_to: MigratedTo = load(deps.storage, MIGRATED_TO_KEY)?;
            let migrated_error = Err(StdError::generic_err(format!(
                "This contract has been migrated to {:?}. Only MigratedTo, MigratedFrom queries allowed!",
                migrated_to.contract.address
            )));
            match msg {
                QueryMsg::MigratedTo {} => query_migrated_info(deps, false),
                QueryMsg::MigratedFrom {} => query_migrated_info(deps, true),
                _ => migrated_error
            }
        }
        _ => {
            match msg {
                QueryMsg::GetPrices {} => query_prices(deps),
                QueryMsg::GetChildSnip721 {} => query_child_snip721(deps),
                QueryMsg::MigratedTo {} => query_migrated_info(deps, false),
                QueryMsg::MigratedFrom {} => query_migrated_info(deps, true),
            }
        }
    };
}

fn query_child_snip721(deps: Deps) -> StdResult<Binary> {
    to_binary(&QueryAnswer::ContractInfo(
        ContractInfo {
            address: deps.api.addr_humanize(&load::<CanonicalAddr>(deps.storage, CHILD_SNIP721_ADDRESS_KEY)?)?,
            code_hash: load::<String>(deps.storage, CHILD_SNIP721_CODE_HASH_KEY)?,
        }
    ))
}


/// Returns StdResult<Binary> displaying prices to mint in all acceptable currency denoms
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
pub fn query_prices(deps: Deps) -> StdResult<Binary> {
    to_binary(&QueryAnswer::GetPrices {
        prices: load(deps.storage, PURCHASE_PRICES_KEY)?,
    })
}
