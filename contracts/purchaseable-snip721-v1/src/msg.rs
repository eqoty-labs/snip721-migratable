use cosmwasm_std::{Binary, Coin};
use schemars::JsonSchema;
use secret_toolkit::permit::Permit;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::BatchNftDossierElement;
use snip721_reference_impl::royalties::RoyaltyInfo;
use snip721_reference_impl::token::Metadata;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// optionally initialize using data from another contract. All other params will be ignored,
    /// (besides entropy which should supplied) since they will be migrated from the old contract
    pub migrate_from: Option<MigrateFrom>,
    /// Allowed Coin prices for purchasing a mint
    pub prices: Option<Vec<Coin>>,
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
    Base(snip721_reference_impl::msg::ExecuteMsg),
    Ext(ExecuteMsgExt),
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct MigrateFrom {
    pub address: String,
    pub code_hash: String,
    /// permit for the  used to verify address executing migration is admin
    pub admin_permit: Permit,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct MigrateTo {
    pub address: String,
    pub code_hash: String,
    pub entropy: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsgExt {
    PurchaseMint {},
    /// Set migration secret (using entropy for randomness), and the address of the new contract
    Migrate {
        /// permit used to verify address executing migration is admin
        admin_permit: Permit,
        migrate_to: MigrateTo,
    },
}

// https://github.com/CosmWasm/serde-json-wasm/issues/43#issuecomment-1263097436
#[doc(hidden)]
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    #[allow(unused_extern_crates, clippy::useless_attribute)]
    extern crate serde as _serde;
    #[automatically_derived]
    impl<'de> _serde::Deserialize<'de> for ExecuteMsg {
        fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
        {
            // [1] `_serde::__private::de::Content` is where the problem lies
            let __content = match <serde_cw_value::Value>::deserialize(__deserializer) {
                _serde::__private::Ok(__val) => __val,
                _serde::__private::Err(__err) => {
                    return _serde::__private::Err(__err);
                }
            };
            if let _serde::__private::Ok(__ok) = _serde::__private::Result::map(
                <snip721_reference_impl::msg::ExecuteMsg as _serde::Deserialize>::deserialize(
                    serde_cw_value::ValueDeserializer::<serde_cw_value::DeserializerError>::new(
                        __content.clone(),
                    ),
                ),
                ExecuteMsg::Base,
            ) {
                return _serde::__private::Ok(__ok);
            }
            if let _serde::__private::Ok(__ok) = _serde::__private::Result::map(
                <ExecuteMsgExt as _serde::Deserialize>::deserialize(
                    serde_cw_value::ValueDeserializer::<
                        // [2] Error is also where the problem lies
                        serde_cw_value::DeserializerError,
                    >::new(__content.clone()),
                ),
                ExecuteMsg::Ext,
            ) {
                return _serde::__private::Ok(__ok);
            }
            _serde::__private::Err(_serde::de::Error::custom(
                "data did not match any variant of untagged enum ExecuteMsg",
            ))
        }
    }
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateByMigrationReplyDataMsg {
    pub migrated_instantiate_msg: InstantiateMsg,
    pub migrate_from: MigrateFrom,
    pub mint_count: u32,
    pub secret: Binary,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteAnswer {}

#[derive(Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum QueryMsg {
    Base(snip721_reference_impl::msg::QueryMsg),
    Ext(QueryMsgExt),
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsgExt {
    /// GetPrices returns the purchase price in acceptable coin types.
    GetPrices {},
    /// The new contract can query this to extract all the information.
    ExportMigrationData {
        start_index: Option<u32>,
        max_count: Option<u32>,
        secret: Binary,
    },
}

// todo: remove when resolved
// https://github.com/CosmWasm/serde-json-wasm/issues/43#issuecomment-1263097436
#[doc(hidden)]
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    #[allow(unused_extern_crates, clippy::useless_attribute)]
    extern crate serde as _serde;
    #[automatically_derived]
    impl<'de> _serde::Deserialize<'de> for QueryMsg {
        fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
        {
            // [1] `_serde::__private::de::Content` is where the problem lies
            let __content = match <serde_cw_value::Value>::deserialize(__deserializer) {
                _serde::__private::Ok(__val) => __val,
                _serde::__private::Err(__err) => {
                    return _serde::__private::Err(__err);
                }
            };
            if let _serde::__private::Ok(__ok) = _serde::__private::Result::map(
                <snip721_reference_impl::msg::QueryMsg as _serde::Deserialize>::deserialize(
                    serde_cw_value::ValueDeserializer::<serde_cw_value::DeserializerError>::new(
                        __content.clone(),
                    ),
                ),
                QueryMsg::Base,
            ) {
                return _serde::__private::Ok(__ok);
            }
            if let _serde::__private::Ok(__ok) = _serde::__private::Result::map(
                <QueryMsgExt as _serde::Deserialize>::deserialize(
                    serde_cw_value::ValueDeserializer::<serde_cw_value::DeserializerError>::new(
                        __content.clone(),
                    ),
                ),
                QueryMsg::Ext,
            ) {
                return _serde::__private::Ok(__ok);
            }
            _serde::__private::Err(_serde::de::Error::custom(
                "data did not match any variant of untagged enum QueryMsg",
            ))
        }
    }
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    // GetPrices returns the purchase price in acceptable coin types.
    GetPrices { prices: Vec<Coin> },

    MigrationBatchNftDossier {
        last_mint_index: u32,
        nft_dossiers: Vec<BatchNftDossierElement>,
    },
}
