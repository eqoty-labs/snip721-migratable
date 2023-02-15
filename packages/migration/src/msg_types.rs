use cosmwasm_std::Addr;
use schemars::JsonSchema;
use secret_toolkit::permit::Permit;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateByMigrationMsg {
    pub migrate_from: MigrateFrom,
    pub entropy: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct MigrateFrom {
    pub address: Addr,
    pub code_hash: String,
    /// permit for the  used to verify address executing migration is admin
    pub admin_permit: Permit,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct MigrateTo {
    pub address: Addr,
    pub code_hash: String,
    pub entropy: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ContractInfo {
    pub address: Addr,
    #[serde(default)]
    pub code_hash: String,
}
