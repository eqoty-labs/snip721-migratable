use cosmwasm_std::{
    Binary, CanonicalAddr, ContractInfo, Deps, DepsMut, entry_point, Env, MessageInfo, Response,
    StdError, StdResult,
};
use cw_migratable_contract_std::execute::register_to_notify_on_migration_complete;
use cw_migratable_contract_std::execute::update_migrated_subscriber;
use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
use cw_migratable_contract_std::state::{
    canonicalize, REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS,
};
use snip721_reference_impl::msg::QueryMsg;
use snip721_reference_impl::royalties::StoredRoyaltyInfo;
use snip721_reference_impl::state::{
    DEFAULT_ROYALTY_KEY, may_load, MINTERS_KEY, save,
};

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

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    snip721_reference_impl::contract::query(deps, env, msg)
}
