use cosmwasm_std::{Binary, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, to_binary, WasmMsg};
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;
use snip721_reference_impl::state::{Config, CONFIG_KEY, load, save};

use migration::msg_types::MigrateTo;
use migration::state::{CONTRACT_MODE_KEY, ContractMode, MIGRATED_TO_KEY, MigratedTo};

use crate::contract_migrate::{instantiate_with_migrated_config, migrate, migration_dossier_list, perform_token_migration, query_migrated_info};
use crate::msg::{ExecuteMsg, ExecuteMsgExt, InstantiateMsg, QueryMsg, QueryMsgExt};

const MIGRATE_REPLY_ID: u64 = 1u64;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    return match msg {
        InstantiateMsg::New(init) => init_snip721(&mut deps, &env, info, init),
        InstantiateMsg::Migrate(init) => {
            let migrate_from = init.migrate_from;
            let migrate_msg = ExecuteMsg::Ext(ExecuteMsgExt::Migrate {
                admin_permit: migrate_from.admin_permit,
                migrate_to: MigrateTo {
                    address: env.contract.address.clone(),
                    code_hash: env.contract.code_hash,
                    entropy: init.entropy,
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


            Ok(Response::new()
                .add_submessages([migrate_submessage])
            )
        }
    };
}

pub(crate) fn init_snip721(
    deps: &mut DepsMut,
    env: &Env,
    info: MessageInfo,
    msg: Snip721InstantiateMsg,
) -> StdResult<Response> {
    save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::Running)?;
    let snip721_response = snip721_reference_impl::contract::instantiate(deps, env, info.clone(), msg)
        .unwrap();

    return Ok(snip721_response);
}


#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
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
                migrated_to.contract.address
            )))
        }
    };
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
    return match msg {
        QueryMsg::Base(base_msg) => {
            match mode {
                ContractMode::MigratedOut => {
                    let migrated_to: MigratedTo = load(deps.storage, MIGRATED_TO_KEY)?;
                    let migrated_error = Err(StdError::generic_err(format!(
                        "This contract has been migrated to {:?}. Only TransactionHistory, MigratedTo, MigratedFrom queries allowed!",
                        migrated_to.contract.address
                    )));

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
                _ => snip721_reference_impl::contract::query(deps, env, base_msg)
            }
        }
        QueryMsg::Ext(base_msg) => {
            match base_msg {
                QueryMsgExt::MigratedTo {} => query_migrated_info(deps, false),
                QueryMsgExt::MigratedFrom {} => query_migrated_info(deps, true),
                QueryMsgExt::ExportMigrationData { start_index, max_count, secret } =>
                    migration_dossier_list(deps, &env.block, start_index, max_count, &secret),
            }
        }
    };
}
