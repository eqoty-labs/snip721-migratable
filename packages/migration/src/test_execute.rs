#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Api, ContractInfo, StdError, StdResult, Storage};
    use cosmwasm_std::testing::{mock_dependencies, mock_info};
    use secret_toolkit::serialization::{Bincode2, Serde};

    use crate::execute::register_on_migration_complete_notify_receiver;
    use crate::state::ON_MIGRATION_COMPLETE_NOTIFY_RECEIVER;

    #[test]
    fn register_on_migration_complete_notify_receiver_fails_with_for_non_admin() {
        let mut deps = mock_dependencies();
        let non_admin_info = mock_info("non_admin", &[]);
        let admin = deps.api.addr_canonicalize("admin").unwrap();
        let receiver_address = "addr".to_string();
        let receiver_code_hash = "code_hash".to_string();
        let res = register_on_migration_complete_notify_receiver(
            deps.as_mut(),
            non_admin_info,
            admin,
            receiver_address,
            receiver_code_hash,
        );
        assert!(res.is_err(), "execute didn't fail");
        assert_eq!(
            res.err().unwrap(),
            StdError::generic_err("This is an admin command and can only be run from the admin address")
        );
    }

    #[test]
    fn register_on_migration_complete_notify_receiver_saves_contract() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        let admin = deps.api.addr_canonicalize("admin")?;
        let receiver = ContractInfo {
            address: Addr::unchecked("addr"),
            code_hash: "code_hash".to_string(),
        };
        register_on_migration_complete_notify_receiver(
            deps.as_mut(),
            admin_info,
            admin,
            receiver.address.to_string(),
            receiver.code_hash.to_string(),
        )?;
        let saved_contract: ContractInfo = Bincode2::deserialize(
            &deps.storage.get(ON_MIGRATION_COMPLETE_NOTIFY_RECEIVER).unwrap()
        )?;
        assert_eq!(receiver, saved_contract);
        Ok(())
    }
}
