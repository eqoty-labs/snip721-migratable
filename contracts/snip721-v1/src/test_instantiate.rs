#[cfg(test)]
mod tests {
    use cosmwasm_std::{Coin, MessageInfo, StdError, Uint128};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    use crate::contract::instantiate;
    use crate::msg::InstantiateMsg;

    pub fn instantiate_msg(prices: Vec<Coin>, admin_info: MessageInfo) -> InstantiateMsg {
        InstantiateMsg {
            prices: prices.clone(),
            public_metadata: None,
            private_metadata: None,
            admin: admin_info.sender.to_string(),
            entropy: "".to_string(),
            royalty_info: None
        }
    }

    #[test]
    fn instantiate_with_valid_msg_succeeds() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];

        let admin_info = mock_info("admin", &[]);
        let _minter_info = mock_info("minty", &prices.clone());

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg);

        assert!(
            res.is_ok(),
        );
    }

    #[test]
    fn instantiate_with_no_prices_fails() {
        let prices = vec![];

        let admin_info = mock_info("creator", &[]);
        let _minter_info = mock_info("minty", &prices.clone());

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg);

        assert!(
            res.is_err(),
        );
        assert_eq!(
            res.err().unwrap(),
            StdError::generic_err("No purchase prices were specified")
        );
    }
}
