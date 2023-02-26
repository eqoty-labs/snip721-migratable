use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

use cosmwasm_contract_migratable_std::msg_types::InstantiateByMigrationMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MigratableSnip721InstantiateMsg {
    /// initialize using data from another contract
    Migrate(InstantiateByMigrationMsg),
    /// initialize fresh
    New(Snip721InstantiateMsg),
}
