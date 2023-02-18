use cosmwasm_std::Addr;
use schemars::JsonSchema;
use secret_toolkit::permit::Permit;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplyError {
    StateChangesNotAllowed {
        message: String,
        migrated_to: ContractInfo,
    },
}

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

#[derive(Serialize, Deserialize, Clone, Eq, Debug, PartialEq, JsonSchema)]
pub struct ContractInfo {
    pub address: Addr,
    #[serde(default)]
    pub code_hash: String,
}

impl Into<cosmwasm_std::ContractInfo> for ContractInfo {
    fn into(self) -> cosmwasm_std::ContractInfo {
        cosmwasm_std::ContractInfo {
            address: self.address,
            code_hash: self.code_hash,
        }
    }
}

impl Into<ContractInfo> for cosmwasm_std::ContractInfo {
    fn into(self) -> ContractInfo {
        ContractInfo {
            address: self.address,
            code_hash: self.code_hash,
        }
    }
}
