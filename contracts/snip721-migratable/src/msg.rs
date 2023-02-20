use cosmwasm_std::{Binary, CanonicalAddr};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::BatchNftDossierElement;
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

use migration::msg::{MigratableExecuteMsg, MigratableQueryMsg};
use migration::msg_types::{ContractInfo, InstantiateByMigrationMsg, MigrateFrom};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstantiateMsg {
    /// initialize using data from another contract
    Migrate(InstantiateByMigrationMsg),
    /// initialize fresh
    New(Snip721InstantiateMsg),
}

#[derive(Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ExecuteMsg {
    Base(snip721_reference_impl::msg::ExecuteMsg),
    Migrate(MigratableExecuteMsg),
    Ext(ExecuteMsgExt),
}


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsgExt {
    MigrateTokensIn {
        /// The number of queries to make from the contract being migrated from
        pages: Option<u32>,
        /// The number of tokens to request from the contract being migrated from in each query.
        /// The number returned could be less.
        page_size: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateByMigrationReplyDataMsg {
    pub migrated_instantiate_msg: Snip721InstantiateMsg,
    pub migrate_from: MigrateFrom,
    pub on_migration_complete_notify_receiver: Option<ContractInfo>,
    pub minters: Vec<CanonicalAddr>,
    pub mint_count: u32,
    pub secret: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteAnswer {
    MigrateTokensIn {
        complete: bool,
        next_mint_index: Option<u32>,
        total: Option<u32>,
    },
}

#[derive(Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum QueryMsg {
    Base(snip721_reference_impl::msg::QueryMsg),
    Ext(QueryMsgExt),
    Migrate(MigratableQueryMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsgExt {
    /// The new contract can query this to extract all the information.
    ExportMigrationData {
        start_index: Option<u32>,
        max_count: Option<u32>,
        secret: Binary,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    MigrationBatchNftDossier {
        last_mint_index: u32,
        nft_dossiers: Vec<BatchNftDossierElement>,
    },
}

