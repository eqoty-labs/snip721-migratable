use cosmwasm_std::{Addr, Binary};
use schemars::JsonSchema;
use secret_toolkit::permit::Permit;
use serde::{Deserialize, Serialize};
use snip721_reference_impl::msg::BatchNftDossierElement;
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

use migration::msg_types::{MigrateFrom, MigrateTo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstantiateMsg {
    /// initialize using data from another contract
    Migrate(InstantiateByMigrationMsg),
    /// initialize fresh
    New(Snip721InstantiateMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateByMigrationMsg {
    pub migrate_from: MigrateFrom,
    pub entropy: String,
}


#[derive(Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum ExecuteMsg {
    Base(snip721_reference_impl::msg::ExecuteMsg),
    Ext(ExecuteMsgExt),
}


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsgExt {
    /// Set migration secret (using entropy for randomness), and the address of the new contract
    Migrate {
        /// permit used to verify address executing migration is admin
        admin_permit: Permit,
        migrate_to: MigrateTo,
    },
    MigrateTokensIn {
        /// The number of queries to make from the contract being migrated from
        pages: Option<u32>,
        /// The number of tokens to request from the contract being migrated from in each query.
        /// The number returned could be less.
        page_size: Option<u32>,
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
    pub migrated_instantiate_msg: Snip721InstantiateMsg,
    pub migrate_from: MigrateFrom,
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
    MigratedFrom {},
    MigratedTo {},
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
    MigrationBatchNftDossier {
        last_mint_index: u32,
        nft_dossiers: Vec<BatchNftDossierElement>,
    },
    MigrationInfo {
        /// the address the contract migrated from/to, otherwise none
        address: Option<Addr>,
        /// the code hash of the contract that was migrated from/to, otherwise none
        code_hash: Option<String>,
    },
}
