use cw_migratable_contract_std::msg_types::InstantiateByMigrationMsg;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MigratableSnip721InstantiateMsg {
    /// initialize using data from another contract
    Migrate(InstantiateByMigrationMsg),
    /// initialize fresh
    New {
        instantiate: Snip721InstantiateMsg,
        // the number of contracts that can be registered to be notified of migration
        max_migration_complete_event_subscribers: u8,
    },
}
