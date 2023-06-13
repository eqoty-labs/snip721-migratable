use cosmwasm_std::{
    entry_point, to_binary, Binary, CanonicalAddr, ContractInfo, Deps, DepsMut, Env, MessageInfo,
    Reply, Response, StdError, StdResult, SubMsg, WasmMsg,
};
use cw_migratable_contract_std::execute::check_contract_mode;
use cw_migratable_contract_std::execute::register_to_notify_on_migration_complete;
use cw_migratable_contract_std::execute::{
    build_operation_unavailable_error, update_migrated_subscriber,
};
use cw_migratable_contract_std::msg::{
    MigratableExecuteMsg, MigratableQueryMsg, MigrationListenerExecuteMsg,
};
use cw_migratable_contract_std::msg_types::MigrateTo;
use cw_migratable_contract_std::msg_types::ReplyError::OperationUnavailable;
use cw_migratable_contract_std::query::query_migrated_info;
use cw_migratable_contract_std::state::{
    canonicalize, ContractMode, CONTRACT_MODE, MIGRATED_TO, MIGRATION_COMPLETE_EVENT_SUBSCRIBERS,
    REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS,
};
use schemars::_serde_json::to_string;
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;
use snip721_reference_impl::royalties::StoredRoyaltyInfo;
use snip721_reference_impl::state::{
    load, may_load, save, Config, CONFIG_KEY, DEFAULT_ROYALTY_KEY, MINTERS_KEY,
};

use crate::contract_migrate::{
    instantiate_with_migrated_config, migrate, migration_dossier_list, perform_token_migration,
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
    match msg {
        InstantiateMsg::New {
            instantiate,
            max_migration_complete_event_subscribers,
        } => {
            REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS
                .save(deps.storage, &max_migration_complete_event_subscribers)?;
            init_snip721(&mut deps, &env, info, instantiate)
        }
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
    }
}

pub(crate) fn init_snip721(
    deps: &mut DepsMut,
    env: &Env,
    info: MessageInfo,
    msg: Snip721InstantiateMsg,
) -> StdResult<Response> {
    CONTRACT_MODE.save(deps.storage, &ContractMode::Running)?;
    let snip721_response =
        snip721_reference_impl::contract::instantiate(deps, env, info, msg).unwrap();

    Ok(snip721_response)
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let mode = CONTRACT_MODE.load(deps.storage)?;
    match msg {
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::MigrateTokensIn { page_size } => {
                perform_token_migration(deps, &env, info, mode, config, page_size)
            }
        },
        ExecuteMsg::Base(base_msg) => execute_base_snip721(deps, env, info, mode, base_msg),
        ExecuteMsg::Migrate(ext_msg) => match ext_msg {
            MigratableExecuteMsg::Migrate {
                admin_permit,
                migrate_to,
            } => migrate(deps, env, info, mode, &mut config, admin_permit, migrate_to),
            MigratableExecuteMsg::SubscribeToMigrationCompleteEvent { address, code_hash } => {
                register_to_notify_on_migration_complete(deps, mode, address, code_hash)
            }
        },
        ExecuteMsg::MigrateListener(migrated_msg) => match migrated_msg {
            MigrationListenerExecuteMsg::MigrationCompleteNotification { to, .. } => {
                on_migration_notification(deps, info, mode, to)
            }
        },
    }
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
        ContractMode::Running => update_migrated_dependency(deps, info, mode, to),
        ContractMode::MigrateOutStarted => on_migration_complete(deps, info, mode),
        _ => Err(build_operation_unavailable_error(&mode, None)),
    }
}

pub(crate) fn update_migrated_dependency(
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
    let migrated_to_raw = canonicalize(deps.api, &migrated_to)?;
    let minter_index_to_update = minters.iter().position(|minter| minter == &from_raw);
    if let Some(some_minter_index_to_update) = minter_index_to_update {
        minters[some_minter_index_to_update] = migrated_to_raw.address.clone();
        update = true;
    }

    // only save if the list changed
    if update {
        save(deps.storage, MINTERS_KEY, &minters)?;
    }

    // check if the royalty info is set to a minter. If so update it.
    if let Some(mut stored_royalty_info) =
        may_load::<StoredRoyaltyInfo>(deps.storage, DEFAULT_ROYALTY_KEY)?
    {
        let mut royalties = stored_royalty_info.royalties;
        let royalty_index_to_update = royalties
            .iter()
            .position(|royalty| royalty.recipient == from_raw);
        if let Some(some_royalty_index_to_update) = royalty_index_to_update {
            royalties[some_royalty_index_to_update].recipient = migrated_to_raw.address.clone();
            update = true;
        }
        if update {
            stored_royalty_info.royalties = royalties;
            save(deps.storage, DEFAULT_ROYALTY_KEY, &stored_royalty_info)?
        }
    };
    // update any matching subscribers
    update_migrated_subscriber(deps.storage, &from_raw, &migrated_to_raw)?;

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
    let migrated_to = MIGRATED_TO
        .load(deps.storage)?
        .contract
        .into_humanized(deps.api)?;
    if migrated_to.address != info.sender {
        Err(StdError::generic_err(
            to_string(&OperationUnavailable {
                message: "Only listening for migration complete notifications from the contract being migrated to".to_string(),
            }).unwrap()
        ))
    } else {
        CONTRACT_MODE.save(deps.storage, &ContractMode::MigratedOut)?;
        // notify the contracts registered to be notified on migration complete
        let contracts = MIGRATION_COMPLETE_EVENT_SUBSCRIBERS
            .may_load(deps.storage)?
            .unwrap_or_default();
        let msg = to_binary(
            &MigrationListenerExecuteMsg::MigrationCompleteNotification {
                to: migrated_to,
                data: None,
            },
        )?;
        let sub_msgs: Vec<SubMsg> = contracts
            .into_iter()
            .map(|contract| contract.into_humanized(deps.api).unwrap())
            .map(|contract| {
                let execute = WasmMsg::Execute {
                    msg: msg.clone(),
                    contract_addr: contract.address.to_string(),
                    code_hash: contract.code_hash,
                    funds: vec![],
                };
                SubMsg::new(execute)
            })
            .collect();

        Ok(Response::new().add_submessages(sub_msgs))
    }
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
    let mode = CONTRACT_MODE.load(deps.storage)?;
    match msg {
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
    }
}
