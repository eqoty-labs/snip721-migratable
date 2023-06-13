#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
    };
    use cosmwasm_std::{
        from_binary, Api, CanonicalAddr, Coin, CosmosMsg, Env, OwnedDeps, ReplyOn, StdError,
        StdResult, Uint128, WasmMsg,
    };
    use cw_migratable_contract_std::msg::MigratableExecuteMsg;
    use cw_migratable_contract_std::state::{ContractMode, CONTRACT_MODE};
    use snip721_reference_impl::msg::{
        ExecuteMsg, InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg,
    };
    use snip721_reference_impl::token::Metadata;

    use crate::contract::{instantiate, reply};
    use crate::msg::{InstantiateMsg, InstantiateSelfAndChildSnip721Msg};
    use crate::msg_external::MigratableSnip721InstantiateMsg;
    use crate::state::{
        PurchasableMetadata, ADMIN, CHILD_SNIP721_ADDRESS, CHILD_SNIP721_CODE_HASH,
        PURCHASABLE_METADATA, PURCHASE_PRICES,
    };
    use crate::test_utils::test_utils::{
        admin_msg_info, child_snip721_address, successful_child_snip721_instantiate_reply,
    };

    fn instantiate_successfully() -> StdResult<(
        OwnedDeps<MockStorage, MockApi, MockQuerier>,
        Env,
        InstantiateSelfAndChildSnip721Msg,
    )> {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let admin_info = admin_msg_info();
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let instantiate_new_msg = InstantiateSelfAndChildSnip721Msg {
            admin: Some(admin_info.sender.to_string()),
            prices: prices.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let instantiate_msg = InstantiateMsg::New(instantiate_new_msg.clone());

        instantiate(
            deps.as_mut(),
            env.clone(),
            admin_info.clone(),
            instantiate_msg,
        )?;
        Ok((deps, env, instantiate_new_msg))
    }

    #[test]
    fn instantiate_with_valid_msg_saves_all_to_state() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
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
        let snip721_code_hash = "test_code_hash".to_string();

        let admin_info = admin_msg_info();

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateMsg::New(InstantiateSelfAndChildSnip721Msg {
            admin: Some(admin_info.sender.to_string()),
            snip721_code_hash: snip721_code_hash.clone(),
            prices: prices.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        });

        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        );
        assert!(res.is_ok(),);

        let saved_prices = PURCHASE_PRICES.load(deps.as_ref().storage).unwrap();
        assert_eq!(prices, saved_prices);
        let saved_purchasable_metadata = PURCHASABLE_METADATA.load(deps.as_ref().storage).unwrap();
        assert_eq!(purchasable_metadata, saved_purchasable_metadata);
        let saved_admin: CanonicalAddr = ADMIN.load(deps.as_ref().storage).unwrap();
        assert_eq!(
            deps.api
                .addr_canonicalize(admin_info.sender.as_str())
                .unwrap(),
            saved_admin
        );
        let saved_child_snip721_code_hash =
            CHILD_SNIP721_CODE_HASH.load(deps.as_ref().storage).unwrap();
        assert_eq!(snip721_code_hash, saved_child_snip721_code_hash);

        assert_eq!(
            ContractMode::Running,
            CONTRACT_MODE.load(deps.as_ref().storage).unwrap()
        );
    }

    #[test]
    fn on_instantiate_snip721_reply_child_snip721_address_is_set() {
        let (mut deps, _, _) = instantiate_successfully().unwrap();
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            mock_env(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )
        .unwrap();
        let saved_child_snip721_address =
            CHILD_SNIP721_ADDRESS.load(deps.as_ref().storage).unwrap();

        assert_eq!(
            child_snip721_address,
            deps.api
                .addr_humanize(&saved_child_snip721_address)
                .unwrap()
        );
    }

    #[test]
    fn on_instantiate_snip721_reply_reg_on_migration_complete_notify_receiver_sub_msg_added() {
        let (mut deps, env, instantiate_new_msg) = instantiate_successfully().unwrap();
        let child_snip721_address = child_snip721_address();
        let res = reply(
            deps.as_mut(),
            mock_env(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )
        .unwrap();

        assert_eq!(0, res.messages[0].id);
        assert_eq!(ReplyOn::Never, res.messages[0].reply_on);
        assert!(matches!(
            res.messages[0].msg,
            CosmosMsg::Wasm(WasmMsg::Execute { .. })
        ));

        match &res.messages[0].msg {
            CosmosMsg::Wasm(msg) => match msg {
                WasmMsg::Execute {
                    contract_addr,
                    code_hash,
                    msg,
                    funds,
                } => {
                    assert_eq!(&child_snip721_address, contract_addr);
                    assert_eq!(&instantiate_new_msg.snip721_code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    let execute_msg: MigratableExecuteMsg = from_binary(msg).unwrap();
                    let expected_execute_msg =
                        MigratableExecuteMsg::SubscribeToMigrationCompleteEvent {
                            address: env.contract.address.to_string(),
                            code_hash: env.contract.code_hash,
                        };
                    assert_eq!(expected_execute_msg, execute_msg);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }
    }

    #[test]
    fn on_instantiate_snip721_reply_child_snip721_change_admin_sub_msg_added() {
        let (mut deps, _, instantiate_new_msg) = instantiate_successfully().unwrap();
        let child_snip721_address = child_snip721_address();
        let res = reply(
            deps.as_mut(),
            mock_env(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )
        .unwrap();

        assert_eq!(0, res.messages[1].id);
        assert_eq!(ReplyOn::Never, res.messages[1].reply_on);
        assert!(matches!(
            res.messages[1].msg,
            CosmosMsg::Wasm(WasmMsg::Execute { .. })
        ));

        match &res.messages[1].msg {
            CosmosMsg::Wasm(msg) => match msg {
                WasmMsg::Execute {
                    contract_addr,
                    code_hash,
                    msg,
                    funds,
                } => {
                    assert_eq!(&child_snip721_address, contract_addr);
                    assert_eq!(&instantiate_new_msg.snip721_code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    let execute_msg: ExecuteMsg = from_binary(msg).unwrap();
                    let expected_execute_msg = ExecuteMsg::ChangeAdmin {
                        address: instantiate_new_msg.admin.unwrap(),
                        padding: None,
                    };
                    assert_eq!(expected_execute_msg, execute_msg);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }
    }

    #[test]
    fn instantiate_without_admin_uses_msg_sender() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
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

        let instantiate_msg = InstantiateMsg::New(InstantiateSelfAndChildSnip721Msg {
            admin: None,
            prices: prices.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        });

        let admin_info = admin_msg_info();
        instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        )
        .unwrap();

        let saved_admin: CanonicalAddr = ADMIN.load(deps.as_ref().storage).unwrap();
        assert_eq!(
            deps.api
                .addr_canonicalize(admin_info.sender.as_str())
                .unwrap(),
            saved_admin
        );
    }

    #[test]
    fn instantiate_with_no_prices_fails() {
        let prices = vec![];

        let admin_info = mock_info("creator", &[]);
        let _minter_info = mock_info("minty", &prices.clone());

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateMsg::New(InstantiateSelfAndChildSnip721Msg {
            admin: Some(admin_info.sender.to_string()),
            prices,
            ..InstantiateSelfAndChildSnip721Msg::default()
        });
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        );

        assert!(res.is_err(),);
        assert_eq!(
            res.err().unwrap(),
            StdError::generic_err("No purchase prices were specified")
        );
    }

    #[test]
    fn instantiate_new_adds_submessage_to_instantiate_child_snip721() -> StdResult<()> {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let env = mock_env();
        let admin_info = admin_msg_info();
        let res = instantiate(
            deps.as_mut(),
            env.clone(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg.clone()),
        )?;

        assert_eq!(1, res.messages.len());
        assert_eq!(1u64, res.messages[0].id);
        assert_eq!(ReplyOn::Success, res.messages[0].reply_on);
        assert!(matches!(
            res.messages[0].msg,
            CosmosMsg::Wasm(WasmMsg::Instantiate { .. })
        ));

        match &res.messages[0].msg {
            CosmosMsg::Wasm(msg) => match msg {
                WasmMsg::Instantiate {
                    code_id,
                    code_hash,
                    msg,
                    funds,
                    label,
                } => {
                    assert_eq!(&instantiate_msg.snip721_code_id, code_id);
                    assert_eq!(&instantiate_msg.snip721_code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    assert_eq!(&instantiate_msg.snip721_label, label);
                    let snip721_instantiate_msg: MigratableSnip721InstantiateMsg =
                        from_binary(msg).unwrap();
                    // Note:
                    // We instantiate the child snip721 w/ the dealer as admin to add dealer to its list of minters.
                    // Then a second tx msg in Reply is sent to change the admin to the dealer's admin
                    // So we should make sure the contract address != admins address
                    assert_ne!(env.contract.address, admin_info.sender);
                    let expected_snip721_instantiate_msg = MigratableSnip721InstantiateMsg::New {
                        instantiate: Snip721InstantiateMsg {
                            name: "PurchasableSnip721".to_string(),
                            symbol: "PUR721".to_string(),
                            admin: Some(env.contract.address.to_string()),
                            entropy: instantiate_msg.entropy,
                            royalty_info: instantiate_msg.royalty_info,
                            config: Some(InstantiateConfig {
                                public_token_supply: Some(true),
                                public_owner: Some(true),
                                enable_sealed_metadata: None,
                                unwrapped_metadata_is_private: None,
                                minter_may_update_metadata: None,
                                owner_may_update_metadata: None,
                                enable_burn: Some(false),
                            }),
                            post_init_callback: None,
                            post_init_data: None,
                        },
                        max_migration_complete_event_subscribers: 1,
                    };
                    assert_eq!(expected_snip721_instantiate_msg, snip721_instantiate_msg);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }
        Ok(())
    }
}
