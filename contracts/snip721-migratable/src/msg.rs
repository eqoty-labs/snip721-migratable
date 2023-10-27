use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub instantiate: Snip721InstantiateMsg,
    // the number of contracts that can be registered to be notified of migration
    pub max_migration_complete_event_subscribers: u8,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ExecuteMsg {
    Base(Box<snip721_reference_impl::msg::ExecuteMsg>),
    Migrate(MigratableExecuteMsg),
    MigrateListener(MigrationListenerExecuteMsg),
}
