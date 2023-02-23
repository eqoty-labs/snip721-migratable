use cosmwasm_contract_migratable_std::execute::register_to_notify_on_migration_complete;
use cosmwasm_contract_migratable_std::msg::MigratableQueryAnswer::MigrationInfo;
use cosmwasm_contract_migratable_std::msg::MigratableQueryMsg::MigratedTo;
use cosmwasm_contract_migratable_std::msg::{
    MigratableExecuteMsg, MigratableQueryAnswer, MigratableQueryMsg, MigrationListenerExecuteMsg,
};
use cosmwasm_contract_migratable_std::msg_types::MigrateTo;
use cosmwasm_contract_migratable_std::msg_types::ReplyError::StateChangesNotAllowed;
use cosmwasm_contract_migratable_std::state::{
    ContractMode, MigratedToState, CONTRACT_MODE_KEY, MIGRATED_TO_KEY,
};
use cosmwasm_std::{
    entry_point, to_binary, Binary, CanonicalAddr, ContractInfo, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdError, StdResult, SubMsg, WasmMsg,
};
use schemars::_serde_json::to_string;
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;
use snip721_reference_impl::state::{load, may_load, save, Config, CONFIG_KEY, MINTERS_KEY};

use crate::contract_migrate::{
    instantiate_with_migrated_config, migrate, migration_dossier_list, perform_token_migration,
    query_migrated_info,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, QueryMsgExt};

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
            let migrate_msg = ExecuteMsg::Migrate(MigratableExecuteMsg::Migrate {
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

            let migrate_submessage = SubMsg::reply_on_success(migrate_wasm_msg, MIGRATE_REPLY_ID);

            Ok(Response::new().add_submessages([migrate_submessage]))
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
    let snip721_response =
        snip721_reference_impl::contract::instantiate(deps, env, info.clone(), msg).unwrap();

    return Ok(snip721_response);
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let mode = load(deps.storage, CONTRACT_MODE_KEY)?;
    return match mode {
        ContractMode::MigrateDataIn => perform_token_migration(deps, &env, info, config, msg),
        ContractMode::Running => match msg {
            ExecuteMsg::Base(base_msg) => {
                snip721_reference_impl::contract::execute(deps, env, info, base_msg)
            }
            ExecuteMsg::Migrate(ext_msg) => match ext_msg {
                MigratableExecuteMsg::Migrate {
                    admin_permit,
                    migrate_to,
                } => migrate(deps, env, info, &mut config, admin_permit, migrate_to),
                MigratableExecuteMsg::RegisterToNotifyOnMigrationComplete {
                    address,
                    code_hash,
                } => register_to_notify_on_migration_complete(
                    deps,
                    info,
                    config.admin,
                    address,
                    code_hash,
                ),
            },
            ExecuteMsg::MigrateListener(migrated_msg) => match migrated_msg {
                MigrationListenerExecuteMsg::MigrationCompleteNotification { from } => {
                    update_migrated_minter(deps, from)
                }
            },
            _ => Err(StdError::generic_err(
                "Operation not allowed allowed in ContractMode::Running",
            )),
        },
        ContractMode::MigrateOutStarted => match msg {
            ExecuteMsg::MigrateListener(migrated_msg) => match migrated_msg {
                MigrationListenerExecuteMsg::MigrationCompleteNotification { .. } => {
                    on_migration_complete(deps, info)
                }
            },
            _ => no_state_changes_allowed(deps),
        },
        ContractMode::MigratedOut => no_state_changes_allowed(deps),
    };
}

fn update_migrated_minter(deps: DepsMut, from: ContractInfo) -> StdResult<Response> {
    let mut minters: Vec<CanonicalAddr> = may_load(deps.storage, MINTERS_KEY)?.unwrap_or_default();
    let mut update = false;
    let from_raw = deps.api.addr_canonicalize(from.address.as_str())?;
    let minter_index_to_update = minters.iter().position(|minter| minter == &from_raw);
    if let Some(some_minter_index_to_update) = minter_index_to_update {
        // Since anyone can execute a MigrationCompleteNotification. We do not include the migrated_to
        // info in the message only migrated_from. As someone could potentially exploit that to set
        // their own minter.
        // To make sure that does not happen, query the contract that is being migrated from
        // to find out where it is being migrated to.
        let migrated_to: MigratableQueryAnswer = deps
            .querier
            .query_wasm_smart(from.code_hash, from.address, &MigratedTo {})
            .unwrap();
        if let MigrationInfo(Some(migrated_to)) = migrated_to {
            minters[some_minter_index_to_update] =
                deps.api.addr_canonicalize(migrated_to.address.as_str())?;
            update = true;
        }
    }

    // only save if the list changed
    if update {
        save(deps.storage, MINTERS_KEY, &minters)?;
    }
    Ok(Response::new())
}

fn on_migration_complete(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let migrated_to: MigratedToState = load(deps.storage, MIGRATED_TO_KEY)?;
    return if migrated_to.contract.address != info.sender {
        Err(StdError::generic_err(
            to_string(&StateChangesNotAllowed {
                message: "Only listening for migration complete notifications from the contract being migrated to".to_string(),
                migrated_to: migrated_to.contract,
            }).unwrap()
        ))
    } else {
        save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::MigratedOut)?;
        Ok(Response::new())
    };
}

fn no_state_changes_allowed(deps: DepsMut) -> StdResult<Response> {
    let migrated_to: MigratedToState = load(deps.storage, MIGRATED_TO_KEY)?;
    Err(StdError::generic_err(
        to_string(&StateChangesNotAllowed {
            message: "This contract has been migrated. No further state changes are allowed!"
                .to_string(),
            migrated_to: migrated_to.contract,
        })
        .unwrap(),
    ))
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
                    let migrated_to: MigratedToState = load(deps.storage, MIGRATED_TO_KEY)?;
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
                _ => snip721_reference_impl::contract::query(deps, env, base_msg),
            }
        }
        QueryMsg::Ext(base_msg) => match base_msg {
            QueryMsgExt::ExportMigrationData {
                start_index,
                max_count,
                secret,
            } => migration_dossier_list(deps, &env.block, start_index, max_count, &secret),
        },
        QueryMsg::Migrate(migrate_msg) => match migrate_msg {
            MigratableQueryMsg::MigratedTo {} => query_migrated_info(deps, false),
            MigratableQueryMsg::MigratedFrom {} => query_migrated_info(deps, true),
        },
    };
}
