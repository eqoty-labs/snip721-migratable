use cosmwasm_std::{CanonicalAddr, Coin};
use secret_toolkit::storage::Item;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::token::Metadata;

/// storage for this contract's admin address:
pub static ADMIN: Item<CanonicalAddr> = Item::new(b"admin");
/// storage for the address of this contract's child snip721 contract: CodeInfo
pub static CHILD_SNIP721_CODE_HASH: Item<String> = Item::new(b"childSnip721CodeHash");
/// storage for the address of this contract's child snip721 contract: CanonicalAddr
pub static CHILD_SNIP721_ADDRESS: Item<CanonicalAddr> = Item::new(b"childSnip721Addr");
pub static PURCHASE_PRICES: Item<Vec<Coin>> = Item::new(b"prices");
/// storage for the PurchasableMetadata used for every purchased mint
pub static PURCHASABLE_METADATA: Item<PurchasableMetadata> = Item::new(b"purMetadata");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PurchasableMetadata {
    /// optional public metadata that can be seen by everyone
    pub public_metadata: Option<Metadata>,
    /// optional private metadata that can only be seen by the owner and whitelist
    pub private_metadata: Option<Metadata>,
}
