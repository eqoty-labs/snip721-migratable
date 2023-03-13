use cosmwasm_std::{Addr, Coin};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::token::Metadata;

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
