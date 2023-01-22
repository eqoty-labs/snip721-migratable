#[cfg(test)]
mod tests {
    use cosmwasm_std::{BankMsg, Coin, CosmosMsg, Deps, DepsMut, from_binary, MessageInfo, StdError, Uint128, WasmMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    use crate::contract::{execute, instantiate, query};
    use crate::msg::{ExecuteMsg, ExecuteMsgExt, InstantiateMsg, QueryMsg};

    pub fn instantiate_msg(prices: Vec<Coin>, admin_info: MessageInfo) -> InstantiateMsg {
        InstantiateMsg {
            prices: prices.clone(),
            public_metadata: None,
            private_metadata: None,
            admin: admin_info.sender.to_string(),
            entropy: "".to_string(),
            royalty_info: None,
        }
    }

    pub fn set_viewing_key(deps: DepsMut, viewing_key: String, message_info: MessageInfo) {
        let set_view_key_msg = ExecuteMsg::Base(snip721_reference_impl::msg::ExecuteMsg::SetViewingKey {
            key: viewing_key.clone(),
            padding: None,
        });
        let res = execute(deps, mock_env(), message_info.clone(), set_view_key_msg);
        assert!(
            res.is_ok(),
            "execute failed: {}",
            res.err().unwrap()
        );
    }

    pub fn get_tokens(deps: Deps, viewing_key: String, message_info: MessageInfo) -> Vec<String> {
        let query_msg = QueryMsg::Base(
            snip721_reference_impl::msg::QueryMsg::Tokens {
                owner: message_info.sender.to_string(),
                viewer: None,
                viewing_key: Some(viewing_key.clone()),
                start_after: None,
                limit: None,
            }
        );
        let query_res = query(deps, mock_env(), query_msg);

        assert!(
            query_res.is_ok(),
            "query failed: {}",
            query_res.err().unwrap()
        );
        let query_answer: snip721_reference_impl::msg::QueryAnswer = from_binary(&query_res.unwrap()).unwrap();
        return match query_answer {
            snip721_reference_impl::msg::QueryAnswer::TokenList { tokens } => tokens,
            _ => panic!("unexpected"),
        };
    }

    #[test]
    fn purchase_and_mint_successfully_w_correct_denom_w_correct_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &prices.clone());
        let pay_to_addr = admin_info.sender.clone();

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg).unwrap();

        // there should be one message
        assert_eq!(exec_purchase_res.messages.len(), 1);
        // the message should be a Bank Send message
        println!("{:#?}", exec_purchase_res.messages[0].msg);
        assert!(matches!(exec_purchase_res.messages[0].msg, CosmosMsg::Bank(BankMsg::Send{ .. })));
        // the Bank Send message should have the price of one purchase being sent to admin
        match &exec_purchase_res.messages[0].msg {
            CosmosMsg::Bank(msg) => {
                match msg {
                    BankMsg::Send { to_address: contract_addr, amount: funds, .. } => {
                        assert_eq!(contract_addr, &pay_to_addr);
                        assert_eq!(funds, &prices);
                    }
                    _ => panic!("unexpected"),
                }
            }
            _ => panic!("unexpected"),
        }

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn purchase_and_mint_fails_w_no_sent_funds() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let invalid_funds = &[];

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchase requires one coin denom to be sent with transaction, {} were sent.", invalid_funds.len()),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_correct_denom_w_insufficient_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: c.clone().denom, amount: c.amount - Uint128::from(1u8) }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchase price in {} is {}, but {} was sent",
                        prices[0].denom,
                        prices[0].amount,
                        invalid_funds[0]),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }


    #[test]
    fn purchase_and_mint_fails_w_correct_denom_w_excessive_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: c.clone().denom, amount: c.amount + Uint128::from(1u8) }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchase price in {} is {}, but {} was sent",
                        prices[0].denom,
                        prices[0].amount,
                        invalid_funds[0]),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_wrong_denom_w_correct_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: "`atom`".to_string(), amount: c.amount }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchasing in denom:{} is not allowed", invalid_funds[0].denom),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_wrong_denom_w_insufficient_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: "`atom`".to_string(), amount: c.amount - Uint128::from(1u8) }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchasing in denom:{} is not allowed", invalid_funds[0].denom),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_wrong_denom_w_excessive_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: "`atom`".to_string(), amount: c.amount + Uint128::from(1u8) }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchasing in denom:{} is not allowed", invalid_funds[0].denom),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_multiple_coins_w_correct_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
            Coin {
                amount: Uint128::new(100),
                denom: "`SCRT`".to_string(),
            },
        ];
        let invalid_funds = prices.clone();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &prices.clone());

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchase requires one coin denom to be sent with transaction, {} were sent.", invalid_funds.len()),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_multiple_coins_w_insufficient_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
            Coin {
                amount: Uint128::new(100),
                denom: "`SCRT`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: c.clone().denom, amount: c.amount - Uint128::from(1u8) }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchase requires one coin denom to be sent with transaction, {} were sent.", invalid_funds.len()),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn purchase_and_mint_fails_w_multiple_coins_excessive_amount() {
        let prices = vec![
            Coin {
                amount: Uint128::new(100),
                denom: "`uscrt`".to_string(),
            },
            Coin {
                amount: Uint128::new(100),
                denom: "`SCRT`".to_string(),
            },
        ];
        let invalid_funds = prices.clone().iter()
            .map(|c| Coin { denom: c.clone().denom, amount: c.amount + Uint128::from(1u8) }).collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
        let _res = instantiate(deps.as_mut(), mock_env(), admin_info.clone(), instantiate_msg).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(deps.as_mut(), mock_env(), minter_info.clone(), exec_purchase_msg);

        assert!(
            exec_purchase_res.is_err(),
            "execute didn't fail"
        );
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(
                format!("Purchase requires one coin denom to be sent with transaction, {} were sent.", invalid_funds.len()),
            )
        );

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 0);
    }
}
