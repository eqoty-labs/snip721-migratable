use schemars::JsonSchema;
use secret_toolkit::permit::Permit;
use serde::{Deserialize, Serialize};

use crate::msg_types::MigrateTo;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationExecuteMsg {
    /// Set migration secret (using entropy for randomness), and the address of the new contract
    Migrate {
        /// permit used to verify address executing migration is admin
        admin_permit: Permit,
        migrate_to: MigrateTo,
    },
    /// Sets a contract that should be notified when this contract completes the migration process
    RegisterOnMigrationCompleteNotifyReceiver {
        address: String,
        code_hash: String,
    },
}
