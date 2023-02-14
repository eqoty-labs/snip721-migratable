use serde::{Deserialize, Serialize};
use snip721_reference_impl::token::Metadata;

/// storage key this contract's admin address: CanonicalAddr
pub const ADMIN_KEY: &[u8] = b"admin";
/// storage key for the address of this contract's child snip721 contract: ContractInfo
pub const CHILD_SNIP721_KEY: &[u8] = b"childsnip721";
/// storage key for allowed Coin prices for purchasing a mint: Vec<Coin>
pub const PURCHASE_PRICES_KEY: &[u8] = b"prices";
/// storage key for the PurchasableMetadata used for every purchased mint
pub const PURCHASABLE_METADATA_KEY: &[u8] = b"pur_metadata";
/// storage key for current ContractMode
pub const CONTRACT_MODE_KEY: &[u8] = b"pur_contract_mode";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PurchasableMetadata {
    /// optional public metadata that can be seen by everyone
    pub public_metadata: Option<Metadata>,
    /// optional private metadata that can only be seen by the owner and whitelist
    pub private_metadata: Option<Metadata>,
}
