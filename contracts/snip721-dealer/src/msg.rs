use cosmwasm_contract_migratable_std::msg::{
    MigratableExecuteMsg, MigratableQueryMsg, MigrationListenerExecuteMsg,
};
use cosmwasm_contract_migratable_std::msg_types::{InstantiateByMigrationMsg, MigrateFrom};
use cosmwasm_std::{Binary, Coin, ContractInfo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::royalties::RoyaltyInfo;
use snip721_reference_impl::token::Metadata;

use crate::msg_types::DealerState;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstantiateMsg {
    /// initialize using data from another contract
    Migrate(InstantiateByMigrationMsg),
    /// initialize fresh
    New(InstantiateSelfAndChildSnip721Msg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateSelfAndChildSnip721Msg {
    /// the code hash used to instantiate this contract's child snip721 contract
    pub snip721_code_hash: String,
    /// the code hash used to instantiate this contract's child snip721 contract
    pub snip721_code_id: u64,
    /// the label used to instantiate this contract's child snip721 contract
    pub snip721_label: String,
    /// Allowed Coin prices for purchasing a mint
    pub prices: Vec<Coin>,
    /// optional public metadata that can be seen by everyone
    pub public_metadata: Option<Metadata>,
    /// optional private metadata that can only be seen by the owner and whitelist
    pub private_metadata: Option<Metadata>,

    // Selected fields from Snip721InstantiateMsg below
    /// optional admin address, env.message.sender if missing
    pub admin: Option<String>,
    /// entropy used for prng seed
    pub entropy: String,
    /// optional royalty information to use as default when RoyaltyInfo is not provided to a
    /// minting function
    pub royalty_info: Option<RoyaltyInfo>,
}

#[derive(Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ExecuteMsg {
    Dealer(DealerExecuteMsg),
    Migrate(MigratableExecuteMsg),
    MigrateListener(MigrationListenerExecuteMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DealerExecuteMsg {
    // Purchase a nft mint
    PurchaseMint {},
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateByMigrationReplyDataMsg {
    pub dealer_state: DealerState,
    pub migrate_from: MigrateFrom,
    pub on_migration_complete_notify_receiver: Vec<ContractInfo>,
    pub secret: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteAnswer {}

#[derive(Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Dealer(DealerQueryMsg),
    Migrate(MigratableQueryMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DealerQueryMsg {
    /// GetPrices returns the purchase price in acceptable coin types.
    GetPrices {},
    GetChildSnip721 {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    // GetPrices returns the purchase price in acceptable coin types.
    GetPrices { prices: Vec<Coin> },
    ContractInfo(ContractInfo),
}
