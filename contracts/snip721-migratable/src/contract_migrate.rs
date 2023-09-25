use cosmwasm_std::{entry_point, DepsMut, Empty, Env, Response, StdResult};
use cw_migratable_contract_std::execute::broadcast_migration_complete_notification;
use cw_migratable_contract_std::state::MIGRATION_COMPLETE_EVENT_SUBSCRIBERS;

#[entry_point]
pub fn migrate(deps: DepsMut, env: Env, _msg: Empty) -> StdResult<Response> {
    let contracts_to_notify = MIGRATION_COMPLETE_EVENT_SUBSCRIBERS
        .load(deps.storage)?
        .into_iter()
        .map(|c| c.into_humanized(deps.api))
        .collect::<StdResult<Vec<_>>>()?;
    broadcast_migration_complete_notification(
        deps.as_ref(),
        &env.contract,
        contracts_to_notify,
        None,
    )
}
