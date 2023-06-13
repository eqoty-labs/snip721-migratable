#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cw_migratable_contract_std::state::{ContractMode, CONTRACT_MODE};

    use crate::contract::instantiate;
    use crate::test_utils::test_utils::instantiate_msg;

    #[test]
    fn instantiate_with_valid_msg_succeeds() {
        let admin_info = mock_info("admin", &[]);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(admin_info.clone());
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        );

        assert!(res.is_ok(),);

        assert_eq!(
            ContractMode::Running,
            CONTRACT_MODE.load(deps.as_ref().storage).unwrap()
        );
    }
}
