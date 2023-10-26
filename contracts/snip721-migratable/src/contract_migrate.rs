use cosmwasm_std::{entry_point, DepsMut, Empty, Env, ReplyOn, Response, StdResult};
use cw_migratable_contract_std::execute::create_broadcast_migration_complete_notification_msgs;
use cw_migratable_contract_std::state::MIGRATION_COMPLETE_EVENT_SUBSCRIBERS;

#[entry_point]
pub fn migrate(deps: DepsMut, env: Env, _msg: Empty) -> StdResult<Response> {
    let contracts_to_notify = MIGRATION_COMPLETE_EVENT_SUBSCRIBERS
        .load(deps.storage)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.into_humanized(deps.api))
        .collect::<StdResult<Vec<_>>>()?;
    let msgs = create_broadcast_migration_complete_notification_msgs(
        deps.as_ref(),
        ReplyOn::Never,
        0,
        &env.contract,
        contracts_to_notify,
        None,
    )?;
    Ok(Response::new().add_submessages(msgs))
}
