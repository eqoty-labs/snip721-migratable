#[cfg(test)]
mod tests {
    use cosmwasm_contract_migratable_std::execute::build_operation_unavailable_error;
    use cosmwasm_contract_migratable_std::state::{ContractMode, CONTRACT_MODE};
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::StdResult;
    use snip721_reference_impl::state::save;

    use strum::IntoEnumIterator;

    use crate::contract::query;
    use crate::msg::{DealerQueryMsg, QueryMsg};

    #[test]
    fn query_child_snip721_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let msg = QueryMsg::Dealer(DealerQueryMsg::GetChildSnip721 {});
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
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

    #[test]
    fn query_prices_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let msg = QueryMsg::Dealer(DealerQueryMsg::GetPrices {});
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
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
