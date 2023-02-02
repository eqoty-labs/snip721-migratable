use cosmwasm_std::{BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, to_binary, WasmMsg};
use snip721_reference_impl::contract::mint;
use snip721_reference_impl::msg::{ContractStatus, InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg};
use snip721_reference_impl::state::{Config, CONFIG_KEY, load, save};

use crate::contract_migrate::{instantiate_with_migrated_config, migrate, migration_dossier_list, perform_token_migration, query_migrated_info};
use crate::msg::{ExecuteMsg, ExecuteMsgExt, InstantiateMsg, MigrateTo, QueryAnswer, QueryMsg, QueryMsgExt};
use crate::state::{CONTRACT_MODE_KEY, ContractMode, MIGRATED_TO_KEY, MigratedTo, PURCHASABLE_METADATA_KEY, PurchasableMetadata, PURCHASE_PRICES_KEY};

const MIGRATE_REPLY_ID: u64 = 1u64;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    if msg.migrate_from.is_none() {
        return init_snip721(&mut deps, &env, info, msg);
    } else {
        let migrate_from = msg.migrate_from.unwrap();
        let migrate_msg = ExecuteMsg::Ext(ExecuteMsgExt::Migrate {
            admin_permit: migrate_from.admin_permit,
            migrate_to: MigrateTo {
                address: env.contract.address.clone(),
                code_hash: env.contract.code_hash,
                entropy: msg.entropy,
            },
        });
        let migrate_wasm_msg: WasmMsg = WasmMsg::Execute {
            contract_addr: migrate_from.address.to_string(),
            code_hash: migrate_from.code_hash,
            msg: to_binary(&migrate_msg)?,
            funds: vec![],
        };

        let migrate_submessage = SubMsg::reply_on_success(
            migrate_wasm_msg,
            MIGRATE_REPLY_ID,
        );


        return Ok(Response::new()
            .add_submessages([migrate_submessage])
        );
    }
}

pub(crate) fn init_snip721(
    deps: &mut DepsMut,
    env: &Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let prices = msg.prices.unwrap();
    if prices.len() == 0 {
        return Err(StdError::generic_err(format!(
            "No purchase prices were specified"
        )));
    }
    save(deps.storage, PURCHASE_PRICES_KEY, &prices)?;
    save(deps.storage, PURCHASABLE_METADATA_KEY,
         &PurchasableMetadata {
             public_metadata: msg.public_metadata,
             private_metadata: msg.private_metadata,
         })?;
    save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::Running)?;
    let instantiate_msg = Snip721InstantiateMsg {
        name: "PurchasableSnip721".to_string(),
        symbol: "PUR721".to_string(),
        admin: msg.admin.clone(),
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
    let snip721_response = snip721_reference_impl::contract::instantiate(deps, env, info.clone(), instantiate_msg)
        .unwrap();

    // clear the data (that contains the secret) which would be set when init_snip721 is called
    // from reply as part of the migration process
    // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
    return Ok(snip721_response);
}


#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let mut deps = deps;
    let mode = load(deps.storage, CONTRACT_MODE_KEY)?;
    return match mode {
        ContractMode::MigrateDataIn => {
            perform_token_migration(deps, &env, info, config, msg)
        }
        ContractMode::Running => {
            match msg {
                ExecuteMsg::Base(base_msg) => {
                    snip721_reference_impl::contract::execute(deps, env, info, base_msg)
                }
                ExecuteMsg::Ext(ext_msg) => match ext_msg {
                    ExecuteMsgExt::PurchaseMint { .. } => {
                        purchase_and_mint(&mut deps, env, info, &mut config)
                    }
                    ExecuteMsgExt::Migrate { admin_permit, migrate_to } =>
                        migrate(deps, env, info, &mut config, admin_permit, migrate_to),
                    ExecuteMsgExt::MigrateTokensIn { .. } => {
                        Err(StdError::generic_err(
                            "MigrateTokensIn msg is allowed when in ContractMode:MigrateDataIn",
                        ))
                    }
                },
            }
        }
        ContractMode::MigratedOut => {
            let migrated_to: MigratedTo = load(deps.storage, MIGRATED_TO_KEY)?;
            Err(StdError::generic_err(format!(
                "This contract has been migrated to {:?}. No further state changes are allowed!",
                migrated_to.address
            )))
        }
    };
}

fn purchase_and_mint(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    config: &mut Config,
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
    let pay_to_addr = deps.api.addr_humanize(&config.admin).unwrap();
    let send_funds_messages = vec![CosmosMsg::Bank(BankMsg::Send {
        to_address: pay_to_addr.to_string(),
        amount: info.funds.clone(),
    })];
    let admin_addr = deps.api.addr_humanize(&config.admin).unwrap();
    let purchasable_metadata: PurchasableMetadata = load(deps.storage, PURCHASABLE_METADATA_KEY)?;
    let mint_result = mint(
        deps,
        &env,
        &admin_addr,
        config,
        ContractStatus::Normal.to_u8(),
        None,
        Some(sender.to_string()),
        purchasable_metadata.public_metadata,
        purchasable_metadata.private_metadata,
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
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        MIGRATE_REPLY_ID => instantiate_with_migrated_config(deps, &env, msg),
        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let mode = load(deps.storage, CONTRACT_MODE_KEY)?;
    return match mode {
        ContractMode::MigratedOut => {
            let migrated_to: MigratedTo = load(deps.storage, MIGRATED_TO_KEY)?;
            let migrated_error = Err(StdError::generic_err(format!(
                "This contract has been migrated to {:?}. Only TransactionHistory, MigratedTo, MigratedFrom queries allowed!",
                migrated_to.address
            )));
            match msg {
                QueryMsg::Base(base_msg) => {
                    let is_tx_history_query = match &base_msg {
                        snip721_reference_impl::msg::QueryMsg::TransactionHistory { .. } => true,
                        snip721_reference_impl::msg::QueryMsg::WithPermit { permit: _, query } => {
                            match query {
                                snip721_reference_impl::msg::QueryWithPermit::TransactionHistory { .. } => true,
                                _ => false
                            }
                        }
                        _ => false
                    };
                    if is_tx_history_query {
                        snip721_reference_impl::contract::query(deps, env, base_msg)
                    } else {
                        migrated_error
                    }
                }
                QueryMsg::Ext(base_msg) => {
                    match base_msg {
                        QueryMsgExt::MigratedTo {} => query_migrated_info(deps, false),
                        QueryMsgExt::MigratedFrom {} => query_migrated_info(deps, true),
                        QueryMsgExt::ExportMigrationData { start_index, max_count, secret } =>
                            migration_dossier_list(deps, &env.block, start_index, max_count, &secret),
                        _ => migrated_error
                    }
                }
            }
        }
        _ => {
            match msg {
                QueryMsg::Base(base_msg) => snip721_reference_impl::contract::query(deps, env, base_msg),
                QueryMsg::Ext(ext_msg) => match ext_msg {
                    QueryMsgExt::GetPrices {} => query_prices(deps),
                    QueryMsgExt::MigratedTo {} => query_migrated_info(deps, false),
                    QueryMsgExt::MigratedFrom {} => query_migrated_info(deps, true),
                    QueryMsgExt::ExportMigrationData { .. } => Err(StdError::generic_err(
                        "This contract has not been migrated yet",
                    ))
                },
            }
        }
    };
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
