use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MigratableSnip721InstantiateMsg {
    pub instantiate: Snip721InstantiateMsg,
    // the number of contracts that can be registered to be notified of migration
    pub max_migration_complete_event_subscribers: u8,
}
