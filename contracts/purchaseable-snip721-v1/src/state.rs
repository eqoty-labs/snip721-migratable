use cosmwasm_std::{Addr, Binary, Coin, Storage};
use cosmwasm_storage::{ReadonlySingleton, singleton, Singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::token::Metadata;

pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Allowed Coin prices for purchasing a mint
    pub prices: Vec<Coin>,
    /// optional public metadata that can be seen by everyone
    pub public_metadata: Option<Metadata>,
    /// optional private metadata that can only be seen by the owner and whitelist
    pub private_metadata: Option<Metadata>,
    pub migration_addr: Option<Addr>,
    pub migration_secret: Option<Binary>,
    pub mode: ContractMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ContractMode {
    Running,
    Migrated,
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}
