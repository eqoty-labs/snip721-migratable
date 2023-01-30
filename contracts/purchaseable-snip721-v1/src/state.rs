use cosmwasm_std::{Addr, Binary, Storage};
use cosmwasm_storage::{ReadonlySingleton, singleton, Singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::token::Metadata;

pub static CONFIG_KEY: &[u8] = b"config";

/// storage key for allowed Coin prices for purchasing a mint: Vec<Coin>
pub const PURCHASE_PRICES_KEY: &[u8] = b"prices";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// optional public metadata that can be seen by everyone
    pub public_metadata: Option<Metadata>,
    /// optional private metadata that can only be seen by the owner and whitelist
    pub private_metadata: Option<Metadata>,
    /// When in ContractMode.MigrateDataIn, it will hold the address being migrated from
    /// When in ContractMode.Running, it will be None
    /// When in ContractMode.MigratedOut, it will hold the address the contract migrated to
    pub migration_addr: Option<Addr>,
    /// When in ContractMode.MigrateDataIn, it will hold the code hash of the contract being migrated from
    /// When in ContractMode.Running, it will be None
    /// When in ContractMode.MigratedOut, it will hold the code hash of the contract that was migrated to
    pub migration_code_hash: Option<String>,
    /// When in ContractMode.MigrateDataIn, it will hold the secret generated by the contract being migrated from
    /// When in ContractMode.Running, it will be None
    /// When in ContractMode.MigratedOut, it will hold the secret needed by another contract to migrate data out
    pub migration_secret: Option<Binary>,
    /// the mint_cnt of the contract being migrated from
    pub migrate_in_mint_cnt: Option<u32>,
    /// the next mint index out of migrate_in_mint_cnt that must be migrated
    pub migrate_in_next_mint_index: Option<u32>,
    pub mode: ContractMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ContractMode {
    MigrateDataIn,
    Running,
    MigratedOut,
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}
