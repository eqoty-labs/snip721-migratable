use cosmwasm_std::{
    entry_point, Binary, ContractInfo, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};
use cw_migratable_contract_std::execute::register_to_notify_on_migration_complete;
use cw_migratable_contract_std::execute::update_migrated_subscriber;
use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
use cw_migratable_contract_std::state::{
    canonicalize, REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS,
};
use snip721_reference_impl::msg::QueryMsg;

use crate::msg::{ExecuteMsg, InstantiateMsg};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS
        .save(deps.storage, &msg.max_migration_complete_event_subscribers)?;
    snip721_reference_impl::contract::instantiate(&mut deps, &env, info, msg.instantiate)
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Base(base_msg) => {
            snip721_reference_impl::contract::execute(deps, env, info, base_msg.as_ref().to_owned())
        }
        ExecuteMsg::Migrate(ext_msg) => match ext_msg {
            MigratableExecuteMsg::SubscribeToMigrationCompleteEvent { address, code_hash } => {
                register_to_notify_on_migration_complete(deps, address, code_hash)
            }
            _ => Err(StdError::generic_err("Unsupported Migrate message")),
        },
        ExecuteMsg::MigrateListener(migrated_msg) => match migrated_msg {
            MigrationListenerExecuteMsg::MigrationCompleteNotification { to, .. } => {
                update_migrated_dependency(deps, info, to)
            }
        },
    }
}

pub(crate) fn update_migrated_dependency(
    deps: DepsMut,
    info: MessageInfo,
    migrated_to: ContractInfo,
) -> StdResult<Response> {
    let from_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let migrated_to_raw = canonicalize(deps.api, &migrated_to)?;
    // update any matching subscribers
    update_migrated_subscriber(deps.storage, &from_raw, &migrated_to_raw)?;
    Ok(Response::new())
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    snip721_reference_impl::contract::query(deps, env, msg)
}
