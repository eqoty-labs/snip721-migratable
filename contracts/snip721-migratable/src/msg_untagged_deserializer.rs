use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
use serde::{de, Deserialize, Deserializer};
use serde::de::{Error, Visitor};

use crate::msg::ExecuteMsg;

struct ExecuteMsgVisitor;

impl<'de> Deserialize<'de> for ExecuteMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(ExecuteMsgVisitor)
    }
}

impl<'de> Visitor<'de> for ExecuteMsgVisitor {
    type Value = ExecuteMsg;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid ExecuteMsg variant")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> where E: Error {
        // Attempt to deserialize into the Base variant
        if let Ok(base_msg) = cosmwasm_std::from_slice::<Box<snip721_reference_impl::msg::ExecuteMsg>>(v) {
            return Ok(ExecuteMsg::Base(base_msg));
        }

        // Attempt to deserialize into the MigratableExecuteMsg variant
        if let Ok(migratable_msg) = cosmwasm_std::from_slice::<MigratableExecuteMsg>(v) {
            return Ok(ExecuteMsg::Migrate(migratable_msg));
        }

        // Attempt to deserialize into the MigrateListener variant
        if let Ok(migration_listener_msg) = cosmwasm_std::from_slice::<MigrationListenerExecuteMsg>(v) {
            return Ok(ExecuteMsg::MigrateListener(migration_listener_msg));
        }

        // If all deserialization attempts fail, return an error
        Err(de::Error::custom("Unsupported Execute message"))
    }
}

