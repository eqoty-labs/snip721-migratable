use cosmwasm_std::{CanonicalAddr, ContractInfo, DepsMut, MessageInfo, Response, StdError, StdResult};
use secret_toolkit::{
    serialization::{Bincode2, Serde},
};

use crate::state::NOTIFY_OF_MIGRATION_RECEIVER_KEY;

pub fn register_on_migration_complete_notify_receiver(
    deps: DepsMut,
    info: MessageInfo,
    admin: CanonicalAddr,
    address: String,
    code_hash: String,
) -> StdResult<Response> {
    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    if admin != sender_raw {
        return Err(StdError::generic_err(
            "This is an admin command and can only be run from the admin address",
        ));
    }
    let contract_info = ContractInfo {
        address: deps.api.addr_validate(address.as_str())?,
        code_hash,
    };
    deps.storage.set(NOTIFY_OF_MIGRATION_RECEIVER_KEY, &Bincode2::serialize(&contract_info)?);
    Ok(Response::new())
}
