use cosmwasm_std::{Addr, Binary, Coin};
use schemars::JsonSchema;
use secret_toolkit::permit::Permit;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::royalties::RoyaltyInfo;
use snip721_reference_impl::token::Metadata;

use migration::msg_types::{ContractInfo, InstantiateByMigrationMsg, MigrateFrom, MigrateTo};

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


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DealerState {
    pub admin: Addr,
    /// Allowed Coin prices for purchasing a mint
    pub prices: Vec<Coin>,
    /// optional public metadata that can be seen by everyone
    pub public_metadata: Option<Metadata>,
    /// optional private metadata that can only be seen by the owner and whitelist
    pub private_metadata: Option<Metadata>,
    /// The snip721 contract's code info for the contract this dealer contract controls
    pub child_snip721_code_hash: String,
    /// The snip721 contract this dealer contract controls
    pub child_snip721_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Purchase a nft mint
    PurchaseMint {},
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

    // todo: rename
    OnMigrationComplete{}
}


#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateByMigrationReplyDataMsg {
    pub dealer_state: DealerState,
    pub migrate_from: MigrateFrom,
    pub secret: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteAnswer {}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// GetPrices returns the purchase price in acceptable coin types.
    GetPrices {},
    GetChildSnip721 {},
    MigratedFrom {},
    MigratedTo {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    // GetPrices returns the purchase price in acceptable coin types.
    GetPrices { prices: Vec<Coin> },
    ContractInfo(ContractInfo),
    MigrationInfo(Option<ContractInfo>),
}
