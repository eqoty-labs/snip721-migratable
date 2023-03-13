#[cfg(test)]
mod tests {
    use cosmwasm_contract_migratable_std::execute::build_operation_unavailable_error;
    use cosmwasm_contract_migratable_std::state::{ContractMode, CONTRACT_MODE};
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{Binary, CanonicalAddr, StdResult};
    use snip721_reference_impl::state::{save, Config, CONFIG_KEY};
    use strum::IntoEnumIterator;

    use crate::contract::query;
    use crate::msg::{QueryMsg, QueryMsgExt};

    #[test]
    fn query_sni721_msg_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let msg = QueryMsg::Base(snip721_reference_impl::msg::QueryMsg::NumTokens { viewer: None });
        let invalid_modes: Vec<ContractMode> =
            vec![ContractMode::MigrateDataIn, ContractMode::MigratedOut];
        for invalid_mode in invalid_modes {
            CONTRACT_MODE.save(deps.as_mut().storage, &invalid_mode)?;
            let res = query(deps.as_ref(), mock_env(), msg.clone());
            assert_eq!(
                res.err().unwrap(),
                build_operation_unavailable_error(&invalid_mode, None)
            );
        }
        Ok(())
    }

    #[test]
    fn query_sni721_msg_fails_when_in_valid_contract_modes_succeeds() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let msg = QueryMsg::Base(snip721_reference_impl::msg::QueryMsg::NumTokens { viewer: None });
        let valid_modes: Vec<ContractMode> =
            vec![ContractMode::Running, ContractMode::MigrateOutStarted];
        save(
            deps.as_mut().storage,
            CONFIG_KEY,
            &Config {
                name: "".to_string(),
                symbol: "".to_string(),
                admin: CanonicalAddr(Binary::from(b"")),
                mint_cnt: 0,
                tx_cnt: 1,
                token_cnt: 0,
                status: 0,
                token_supply_is_public: true,
                owner_is_public: false,
                sealed_metadata_is_enabled: false,
                unwrap_to_private: false,
                minter_may_update_metadata: false,
                owner_may_update_metadata: false,
                burn_is_enabled: false,
            },
        )?;
        for valid_mode in valid_modes {
            CONTRACT_MODE.save(deps.as_mut().storage, &valid_mode)?;
            let res = query(deps.as_ref(), mock_env(), msg.clone());
            assert_eq!(true, res.is_ok(),);
        }
        Ok(())
    }

    #[test]
    fn query_migration_dossier_list_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let msg = QueryMsg::Ext(QueryMsgExt::ExportMigrationData {
            start_index: None,
            max_count: None,
            secret: Default::default(),
        });
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::MigrateOutStarted)
            .collect();
        for invalid_mode in invalid_modes {
            CONTRACT_MODE.save(deps.as_mut().storage, &invalid_mode)?;
            let res = query(deps.as_ref(), mock_env(), msg.clone());
            assert_eq!(
                res.err().unwrap(),
                build_operation_unavailable_error(&invalid_mode, None)
            );
        }
        Ok(())
    }
}
