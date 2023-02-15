#[cfg(test)]
mod tests {
    use cosmwasm_std::{Api, BankMsg, CanonicalAddr, Coin, CosmosMsg, from_binary, StdError, Uint128, WasmMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use snip721_reference_impl::msg::ExecuteMsg as Snip721ExecuteMsg;
    use snip721_reference_impl::state::load;
    use snip721_reference_impl::token::Metadata;

    use crate::contract::{execute, instantiate, reply};
    use crate::msg::{CodeInfo, ExecuteMsg, InstantiateMsg, InstantiateSelfAndChildSnip721Msg};
    use crate::state::{CHILD_SNIP721_ADDRESS_KEY, CHILD_SNIP721_CODE_INFO_KEY, PurchasableMetadata};
    use crate::test_utils::test_utils::{child_snip721_address, successful_child_snip721_instantiate_reply};

    #[test]
    fn purchase_and_mint_successfully_w_correct_denom_w_correct_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let admin_info = mock_info("creator", &[]);
        let mint_recipient_info = mock_info("minty", &prices.clone());
        let pay_to_addr = admin_info.sender.clone();
        let purchasable_metadata = PurchasableMetadata {
            public_metadata: Some(Metadata {
                token_uri: Some("public_metadata_uri".to_string()),
                extension: None,
            }),
            private_metadata: Some(Metadata {
                token_uri: Some("private_metadata_uri".to_string()),
                extension: None,
            }),
        };
        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), mock_env(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            mint_recipient_info.clone(),
            exec_purchase_msg,
        ).unwrap();

        // there should be one message
        assert_eq!(exec_purchase_res.messages.len(), 2);
        // the first message should be a Bank Send message
        assert!(matches!(
            exec_purchase_res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send { .. })
        ));
        // the Bank Send message should have the price of one purchase being sent to admin
        match &exec_purchase_res.messages[0].msg {
            CosmosMsg::Bank(msg) => match msg {
                BankMsg::Send {
                    to_address: contract_addr,
                    amount: funds,
                    ..
                } => {
                    assert_eq!(contract_addr, &pay_to_addr);
                    assert_eq!(funds, &prices);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }
        // the second message should be a Wasm Execute Mint message
        assert!(matches!(
            exec_purchase_res.messages[1].msg,
            CosmosMsg::Wasm(WasmMsg::Execute { .. })
        ));
        match &exec_purchase_res.messages[1].msg {
            CosmosMsg::Wasm(msg) => match msg {
                WasmMsg::Execute {
                    contract_addr, code_hash, msg, funds
                } => {
                    let child_snip721_code_info: CodeInfo = load(deps.as_ref().storage, CHILD_SNIP721_CODE_INFO_KEY).unwrap();
                    let child_snip721_address: CanonicalAddr = load(deps.as_ref().storage, CHILD_SNIP721_ADDRESS_KEY).unwrap();
                    assert_eq!(&deps.api.addr_humanize(&child_snip721_address).unwrap().to_string(), contract_addr);
                    assert_eq!(&child_snip721_code_info.code_hash.to_string(), code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    match from_binary(msg).unwrap() {
                        Snip721ExecuteMsg::MintNft { token_id, owner, public_metadata, private_metadata, serial_number, royalty_info, transferable, memo, padding } => {
                            assert_eq!(None, token_id);
                            assert_eq!(Some(mint_recipient_info.sender.to_string()), owner);
                            assert_eq!(purchasable_metadata.public_metadata, public_metadata);
                            assert_eq!(purchasable_metadata.private_metadata, private_metadata);
                            assert_eq!(None, serial_number);
                            assert_eq!(None, royalty_info);
                            assert_eq!(None, transferable);
                            assert_eq!(None, memo);
                            assert_eq!(None, padding);
                        }
                        _ => panic!("unexpected"),
                    }
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }
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

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchase requires one coin denom to be sent with transaction, {} were sent.",
                invalid_funds.len()
            ), )
        );
    }

    #[test]
    fn purchase_and_mint_fails_w_correct_denom_w_insufficient_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: c.clone().denom,
                amount: c.amount - Uint128::from(1u8),
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchase price in {} is {}, but {} was sent",
                prices[0].denom, prices[0].amount, invalid_funds[0]
            ), )
        );
    }


    #[test]
    fn purchase_and_mint_fails_w_correct_denom_w_excessive_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: c.clone().denom,
                amount: c.amount + Uint128::from(1u8),
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchase price in {} is {}, but {} was sent",
                prices[0].denom, prices[0].amount, invalid_funds[0]
            ), )
        );
    }

    #[test]
    fn purchase_and_mint_fails_w_wrong_denom_w_correct_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: "`atom`".to_string(),
                amount: c.amount,
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchasing in denom:{} is not allowed",
                invalid_funds[0].denom
            ), )
        );
    }

    #[test]
    fn purchase_and_mint_fails_w_wrong_denom_w_insufficient_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: "`atom`".to_string(),
                amount: c.amount - Uint128::from(1u8),
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchasing in denom:{} is not allowed",
                invalid_funds[0].denom
            ), )
        );
    }


    #[test]
    fn purchase_and_mint_fails_w_wrong_denom_w_excessive_amount() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: "`atom`".to_string(),
                amount: c.amount + Uint128::from(1u8),
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchasing in denom:{} is not allowed",
                invalid_funds[0].denom
            ), )
        );
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

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchase requires one coin denom to be sent with transaction, {} were sent.",
                invalid_funds.len()
            ), )
        );
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
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: c.clone().denom,
                amount: c.amount - Uint128::from(1u8),
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchase requires one coin denom to be sent with transaction, {} were sent.",
                invalid_funds.len()
            ), )
        );
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
        let invalid_funds = prices
            .clone()
            .iter()
            .map(|c| Coin {
                denom: c.clone().denom,
                amount: c.amount + Uint128::from(1u8),
            })
            .collect::<Vec<Coin>>();

        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &invalid_funds);

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::PurchaseMint {};
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        );

        assert!(exec_purchase_res.is_err(), "execute didn't fail");
        assert_eq!(
            exec_purchase_res.err().unwrap(),
            StdError::generic_err(format!(
                "Purchase requires one coin denom to be sent with transaction, {} were sent.",
                invalid_funds.len()
            ), )
        );
    }
}
