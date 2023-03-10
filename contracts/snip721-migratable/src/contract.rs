use cosmwasm_contract_migratable_std::execute::build_operation_unavailable_error;
use cosmwasm_contract_migratable_std::execute::check_contract_mode;
use cosmwasm_contract_migratable_std::execute::register_to_notify_on_migration_complete;
use cosmwasm_contract_migratable_std::msg::{
    MigratableExecuteMsg, MigratableQueryMsg, MigrationListenerExecuteMsg,
};
use cosmwasm_contract_migratable_std::msg_types::MigrateTo;
use cosmwasm_contract_migratable_std::msg_types::ReplyError::OperationUnavailable;
use cosmwasm_contract_migratable_std::state::{
    ContractMode, MigratedToState, CONTRACT_MODE_KEY, MIGRATED_TO_KEY,
    NOTIFY_ON_MIGRATION_COMPLETE_KEY,
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
    return match msg {
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::MigrateTokensIn { pages, page_size } => perform_token_migration(
                deps,
                &env,
                info,
                mode,
                config,
                pages.unwrap_or(u32::MAX),
                page_size,
            ),
        },
        ExecuteMsg::Base(base_msg) => execute_base_snip721(deps, env, info, mode, base_msg),
        ExecuteMsg::Migrate(ext_msg) => match ext_msg {
            MigratableExecuteMsg::Migrate {
                admin_permit,
                migrate_to,
            } => migrate(deps, env, info, mode, &mut config, admin_permit, migrate_to),
            MigratableExecuteMsg::RegisterToNotifyOnMigrationComplete { address, code_hash } => {
                register_to_notify_on_migration_complete(
                    deps,
                    info,
                    config.admin,
                    address,
                    code_hash,
                    Some(mode),
                )
            }
        },
        ExecuteMsg::MigrateListener(migrated_msg) => match migrated_msg {
            MigrationListenerExecuteMsg::MigrationCompleteNotification { to, .. } => {
                on_migration_notification(deps, info, mode, to)
            }
        },
    };
}

fn execute_base_snip721(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract_mode: ContractMode,
    msg: snip721_reference_impl::msg::ExecuteMsg,
) -> StdResult<Response> {
    if let Some(contract_mode_error) =
        check_contract_mode(vec![ContractMode::Running], &contract_mode, None)
    {
        return Err(contract_mode_error);
    }
    snip721_reference_impl::contract::execute(deps, env, info, msg)
}

fn on_migration_notification(
    deps: DepsMut,
    info: MessageInfo,
    mode: ContractMode,
    to: ContractInfo,
) -> StdResult<Response> {
    match mode {
        ContractMode::Running => update_migrated_minter(deps, info, mode, to),
        ContractMode::MigrateOutStarted => on_migration_complete(deps, info, mode),
        _ => Err(build_operation_unavailable_error(&mode, None)),
    }
}

pub(crate) fn update_migrated_minter(
    deps: DepsMut,
    info: MessageInfo,
    mode: ContractMode,
    migrated_to: ContractInfo,
) -> StdResult<Response> {
    if let Some(contract_mode_error) = check_contract_mode(vec![ContractMode::Running], &mode, None)
    {
        return Err(contract_mode_error);
    }
    let mut minters: Vec<CanonicalAddr> = may_load(deps.storage, MINTERS_KEY)?.unwrap_or_default();
    let mut update = false;
    let from_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let minter_index_to_update = minters.iter().position(|minter| minter == &from_raw);
    if let Some(some_minter_index_to_update) = minter_index_to_update {
        minters[some_minter_index_to_update] =
            deps.api.addr_canonicalize(migrated_to.address.as_str())?;
        update = true;
    }

    // only save if the list changed
    if update {
        save(deps.storage, MINTERS_KEY, &minters)?;
    }
    Ok(Response::new())
}

pub(crate) fn on_migration_complete(
    deps: DepsMut,
    info: MessageInfo,
    mode: ContractMode,
) -> StdResult<Response> {
    if let Some(contract_mode_error) =
        check_contract_mode(vec![ContractMode::MigrateOutStarted], &mode, None)
    {
        return Err(contract_mode_error);
    }
    let migrated_to: MigratedToState = load(deps.storage, MIGRATED_TO_KEY)?;
    return if migrated_to.contract.address != info.sender {
        Err(StdError::generic_err(
            to_string(&OperationUnavailable {
                message: "Only listening for migration complete notifications from the contract being migrated to".to_string(),
            }).unwrap()
        ))
    } else {
        save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::MigratedOut)?;
        // notify the contracts registered to be notified on migration complete
        let contracts =
            may_load::<Vec<ContractInfo>>(deps.storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY)?
                .unwrap_or_default();
        let msg = to_binary(
            &MigrationListenerExecuteMsg::MigrationCompleteNotification {
                to: migrated_to.contract,
                data: None,
            },
        )?;
        let sub_msgs: Vec<SubMsg> = contracts
            .iter()
            .map(|contract| {
                let execute = WasmMsg::Execute {
                    msg: msg.clone(),
                    contract_addr: contract.address.to_string(),
                    code_hash: contract.code_hash.clone(),
                    funds: vec![],
                };
                SubMsg::new(execute)
            })
            .collect();

        Ok(Response::new().add_submessages(sub_msgs))
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
                        Err(build_operation_unavailable_error(&mode, None))
                    }
                }
                ContractMode::MigrateDataIn => Err(build_operation_unavailable_error(&mode, None)),
                _ => snip721_reference_impl::contract::query(deps, env, base_msg),
            }
        }
        QueryMsg::Ext(base_msg) => match base_msg {
            QueryMsgExt::ExportMigrationData {
                start_index,
                max_count,
                secret,
            } => migration_dossier_list(deps, &env.block, &mode, start_index, max_count, &secret),
        },
        QueryMsg::Migrate(migrate_msg) => match migrate_msg {
            MigratableQueryMsg::MigratedTo {} => query_migrated_info(deps, false),
            MigratableQueryMsg::MigratedFrom {} => query_migrated_info(deps, true),
        },
    };
}
