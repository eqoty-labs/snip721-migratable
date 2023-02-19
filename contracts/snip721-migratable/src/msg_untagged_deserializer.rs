use migration::msg::MigrationExecuteMsg;

use crate::msg::{ExecuteMsg, ExecuteMsgExt, QueryMsg, QueryMsgExt};

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
                <MigrationExecuteMsg as _serde::Deserialize>::deserialize(
                    serde_cw_value::ValueDeserializer::<
                        // [2] Error is also where the problem lies
                        serde_cw_value::DeserializerError,
                    >::new(__content.clone()),
                ),
                ExecuteMsg::Migrate,
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
