#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        from_binary, to_binary, Addr, Api, Binary, BlockInfo, CanonicalAddr, Coin, ContractInfo,
        CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, StdResult,
        SubMsgResponse, SubMsgResult, Timestamp, TransactionInfo, Uint128, WasmMsg,
    };
    use cw_migratable_contract_std::execute::{
        build_operation_unavailable_error, register_to_notify_on_migration_complete,
    };
    use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
    use cw_migratable_contract_std::msg_types::{
        InstantiateByMigrationMsg, MigrateFrom, MigrateTo,
    };
    use cw_migratable_contract_std::state::{
        canonicalize, ContractMode, MigratedFromState, CONTRACT_MODE, MIGRATED_FROM, MIGRATED_TO,
        MIGRATION_COMPLETE_EVENT_SUBSCRIBERS,
    };
    use secret_toolkit::permit::{
        validate, Permit, PermitParams, PermitSignature, PubKey, TokenPermissions,
    };
    use snip721_reference_impl::token::Metadata;
    use strum::IntoEnumIterator;

    use crate::contract::{execute, instantiate, reply};
    use crate::msg::{
        ExecuteMsg, InstantiateByMigrationReplyDataMsg, InstantiateMsg,
        InstantiateSelfAndChildSnip721Msg,
    };
    use crate::msg_types::DealerState;
    use crate::state::{
        PurchasableMetadata, ADMIN, CHILD_SNIP721_ADDRESS, CHILD_SNIP721_CODE_HASH,
        PURCHASABLE_METADATA, PURCHASE_PRICES,
    };
    use crate::test_utils::test_utils::{
        child_snip721_address, child_snip721_code_hash, successful_child_snip721_instantiate_reply,
    };

    const CONTRACT_ADDRESS_0: &str = "secret1rf03820fp8gngzg2w02vd30ns78qkc8rg8dxaq";
    const CONTRACT_ADDRESS_1: &str = "secret18eezxhys9jwku67cm4w84xhnzt4xjj772twz9k";

    fn custom_mock_env_0() -> Env {
        Env {
            block: BlockInfo {
                height: 12_345,
                time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                chain_id: "cosmos-testnet-14002".to_string(),
                random: Some(
                    Binary::from_base64("wLsKdf/sYqvSMI0G0aWRjob25mrIB0VQVjTjDXnDafk=").unwrap(),
                ),
            },
            transaction: Some(TransactionInfo { index: 3 }),
            contract: ContractInfo {
                address: Addr::unchecked(CONTRACT_ADDRESS_0),
                code_hash: "code_hash_0".to_string(),
            },
        }
    }

    fn custom_mock_env_1() -> Env {
        Env {
            block: BlockInfo {
                height: 12_345,
                time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                chain_id: "cosmos-testnet-14002".to_string(),
                random: Some(Binary::from_base64("dGhlIGNha2UgaXMgYSBsaWU=").unwrap()),
            },
            transaction: Some(TransactionInfo { index: 3 }),
            contract: ContractInfo {
                address: Addr::unchecked(CONTRACT_ADDRESS_1),
                code_hash: "code_hash_1".to_string(),
            },
        }
    }

    fn get_admin_permit() -> Permit {
        Permit {
            params: PermitParams {
                allowed_tokens: vec![CONTRACT_ADDRESS_0.to_string()],
                permit_name: "memo_secret1rf03820fp8gngzg2w02vd30ns78qkc8rg8dxaq".to_string(),
                chain_id: "pulsar-2".to_string(),
                permissions: vec![TokenPermissions::History],
            },
            signature: PermitSignature {
                pub_key: PubKey {
                    r#type: "tendermint/PubKeySecp256k1".to_string(),
                    value: Binary::from_base64("A5M49l32ZrV+SDsPnoRv8fH7ivNC4gEX9prvd4RwvRaL").unwrap(),
                },
                signature: Binary::from_base64("hw/Mo3ZZYu1pEiDdymElFkuCuJzg9soDHw+4DxK7cL9rafiyykh7VynS+guotRAKXhfYMwCiyWmiznc6R+UlsQ==").unwrap(),
            },
        }
    }

    fn get_secret_address(deps: Deps, permit: &Permit) -> StdResult<String> {
        validate::<_>(
            deps,
            "test",
            permit,
            CONTRACT_ADDRESS_0.to_string(),
            Some("secret"),
        )
    }

    fn valid_execute_migrate_reply(
        env: Env,
        prices: &Vec<Coin>,
        purchasable_metadata: &PurchasableMetadata,
        admin: &Addr,
        admin_permit: &Permit,
        child_snip721_code_hash: &String,
        child_snip721_address: &Addr,
        secret: &Binary,
    ) -> InstantiateByMigrationReplyDataMsg {
        InstantiateByMigrationReplyDataMsg {
            dealer_state: DealerState {
                prices: prices.clone(),
                public_metadata: purchasable_metadata.public_metadata.clone(),
                private_metadata: purchasable_metadata.private_metadata.clone(),
                admin: admin.clone(),
                child_snip721_code_hash: child_snip721_code_hash.clone(),
                child_snip721_address: child_snip721_address.clone(),
            },
            migrate_from: MigrateFrom {
                address: env.contract.address.clone(),
                code_hash: env.contract.code_hash.clone(),
                admin_permit: admin_permit.clone(),
            },
            migration_complete_event_subscribers: vec![ContractInfo {
                address: Addr::unchecked(child_snip721_address),
                code_hash: child_snip721_code_hash.clone(),
            }],
            secret: secret.clone(),
        }
    }

    pub fn migrate(
        deps: DepsMut,
        admin_permit: &Permit,
        migration_target_addr: &Addr,
        migration_target_code_hash: &str,
    ) -> StdResult<Response> {
        let set_view_key_msg = ExecuteMsg::Migrate(MigratableExecuteMsg::Migrate {
            admin_permit: admin_permit.clone(),
            migrate_to: MigrateTo {
                address: migration_target_addr.clone(),
                code_hash: migration_target_code_hash.to_string(),
                entropy: "magnets, how do they work?".to_string(),
            },
        });
        let res = execute(
            deps,
            custom_mock_env_0(),
            MessageInfo {
                sender: Addr::unchecked(migration_target_addr),
                funds: vec![],
            },
            set_view_key_msg,
        );
        res
    }

    fn assert_is_migration_complete_notification_msg_to_contract(
        cosmos_msg: &CosmosMsg,
        send_to: &ContractInfo,
        migrated_to: &ContractInfo,
    ) {
        return match cosmos_msg {
            CosmosMsg::Wasm(wasm_msg) => match wasm_msg {
                WasmMsg::Execute {
                    contract_addr,
                    code_hash,
                    msg,
                    funds,
                } => {
                    assert_eq!(&send_to.address, contract_addr);
                    assert_eq!(&send_to.code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    let execute_msg: MigrationListenerExecuteMsg = from_binary(msg).unwrap();
                    let expected_execute_msg =
                        MigrationListenerExecuteMsg::MigrationCompleteNotification {
                            to: migrated_to.clone(),
                            data: None,
                        };
                    assert_eq!(expected_execute_msg, execute_msg);
                }

                _ => {}
            },
            _ => {}
        };
    }

    #[test]
    fn instantiate_by_migration_correctly_migrates_dealer_state() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let migrate_from = MigrateFrom {
            address: Addr::unchecked("new_address"),
            code_hash: "code_hash".to_string(),
            admin_permit: admin_permit.clone(),
        };
        let instantiate_msg = InstantiateByMigrationMsg {
            migrate_from: migrate_from.clone(),
            entropy: "Wilson! Wilson!".to_string(),
        };
        let migrate_from_env = custom_mock_env_0();
        let migrate_to_env = custom_mock_env_1();
        assert_ne!(
            migrate_to_env.contract.address.clone(),
            migrate_from_env.contract.address.clone()
        );
        assert_ne!(
            migrate_to_env.contract.code_hash.clone(),
            migrate_from_env.contract.code_hash.clone()
        );

        let res = instantiate(
            deps.as_mut(),
            migrate_to_env.clone(),
            admin_info.clone(),
            InstantiateMsg::Migrate(instantiate_msg.clone()),
        )?;
        assert_eq!(1, res.messages.len());
        assert_eq!(2u64, res.messages[0].id);
        assert_eq!(ReplyOn::Success, res.messages[0].reply_on);
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
                    assert_eq!(&migrate_from.address, contract_addr);
                    assert_eq!(&migrate_from.code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    let execute_msg: ExecuteMsg = from_binary(msg)?;
                    let expected_execute_msg = ExecuteMsg::Migrate(MigratableExecuteMsg::Migrate {
                        admin_permit: admin_permit.clone(),
                        migrate_to: MigrateTo {
                            address: migrate_to_env.contract.address.clone(),
                            code_hash: migrate_to_env.contract.code_hash.clone(),
                            entropy: instantiate_msg.entropy,
                        },
                    });
                    assert_eq!(expected_execute_msg, execute_msg);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }

        // none of the dealer state should be set before reply is called
        let saved_prices = PURCHASE_PRICES.may_load(deps.as_ref().storage)?;
        assert_eq!(None, saved_prices);
        let saved_purchasable_metadata = PURCHASABLE_METADATA.may_load(deps.as_ref().storage)?;
        assert_eq!(None, saved_purchasable_metadata);
        let saved_admin = ADMIN.may_load(deps.as_ref().storage)?;
        assert_eq!(None, saved_admin);
        let saved_child_snip721_code_hash =
            CHILD_SNIP721_CODE_HASH.may_load(deps.as_ref().storage)?;
        assert_eq!(None, saved_child_snip721_code_hash);
        let saved_child_snip721_address = CHILD_SNIP721_ADDRESS.may_load(deps.as_ref().storage)?;
        if let Some(some_saved_child_snip721_address) = saved_child_snip721_address {
            assert_eq!(
                None,
                Some(deps.api.addr_humanize(&some_saved_child_snip721_address)?)
            );
        }
        let saved_contract_mode = CONTRACT_MODE.may_load(deps.as_ref().storage)?;
        assert_eq!(None, saved_contract_mode);

        let migrated_prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let migrated_purchasable_metadata = PurchasableMetadata {
            public_metadata: Some(Metadata {
                token_uri: Some("public_metadata_uri".to_string()),
                extension: None,
            }),
            private_metadata: Some(Metadata {
                token_uri: Some("private_metadata_uri".to_string()),
                extension: None,
            }),
        };
        let migrated_snip721_code_hash = "test_code_hash".to_string();
        let migrated_child_snip721_address = child_snip721_address();
        let migrate_from_secret = &Binary::from(b"some_secret");
        let exec_migrate_reply_msg_data = to_binary(&valid_execute_migrate_reply(
            migrate_from_env.clone(),
            &migrated_prices,
            &migrated_purchasable_metadata,
            &admin_info.sender,
            &admin_permit.clone(),
            &migrated_snip721_code_hash,
            &Addr::unchecked(migrated_child_snip721_address.clone()),
            migrate_from_secret,
        ))?;

        // fake a reply after successful execution of migrate from old version of dealer
        reply(
            deps.as_mut(),
            migrate_to_env.clone(),
            Reply {
                id: 2u64,
                result: SubMsgResult::Ok(SubMsgResponse {
                    data: Some(exec_migrate_reply_msg_data),
                    events: Vec::new(),
                }),
            },
        )?;

        // dealer state should be saved using data from migration
        let saved_prices = PURCHASE_PRICES.may_load(deps.as_ref().storage)?;
        assert_eq!(Some(migrated_prices), saved_prices);
        let saved_purchasable_metadata = PURCHASABLE_METADATA.may_load(deps.as_ref().storage)?;
        assert_eq!(
            Some(migrated_purchasable_metadata),
            saved_purchasable_metadata
        );
        let saved_admin: CanonicalAddr = ADMIN.load(deps.as_ref().storage)?;
        assert_eq!(
            deps.api.addr_canonicalize(admin_info.sender.as_str())?,
            saved_admin
        );
        let saved_child_snip721_code_hash =
            CHILD_SNIP721_CODE_HASH.may_load(deps.as_ref().storage)?;
        assert_eq!(
            Some(migrated_snip721_code_hash),
            saved_child_snip721_code_hash
        );
        let saved_child_snip721_address: CanonicalAddr =
            CHILD_SNIP721_ADDRESS.load(deps.as_ref().storage)?;
        assert_eq!(
            migrated_child_snip721_address,
            deps.api.addr_humanize(&saved_child_snip721_address)?
        );
        let expected_migrated_from = MigratedFromState {
            contract: canonicalize(deps.as_ref().api, &migrate_from_env.contract)?,
            migration_secret: migrate_from_secret.clone(),
        };
        let migrated_from = MIGRATED_FROM.may_load(deps.as_ref().storage)?;
        assert_eq!(Some(expected_migrated_from), migrated_from);
        let saved_contract_mode = CONTRACT_MODE.may_load(deps.as_ref().storage)?;
        assert_eq!(Some(ContractMode::Running), saved_contract_mode);
        Ok(())
    }

    #[test]
    fn migrate_updates_contract_mode_and_migrated_to_state() -> StdResult<()> {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        )?;
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            custom_mock_env_0(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )?;

        let migrate_to_addr_0 = Addr::unchecked("new_address");
        let migrate_to_code_hash_0 = "code_hash";

        migrate(
            deps.as_mut(),
            admin_permit,
            &migrate_to_addr_0,
            migrate_to_code_hash_0,
        )?;

        assert_eq!(
            ContractMode::MigratedOut,
            CONTRACT_MODE.load(deps.as_ref().storage)?
        );
        assert_eq!(
            ContractInfo {
                address: migrate_to_addr_0,
                code_hash: migrate_to_code_hash_0.to_string(),
            },
            MIGRATED_TO
                .load(deps.as_ref().storage)?
                .contract
                .humanize(deps.as_ref().api)?
        );
        Ok(())
    }

    #[test]
    fn migrate_sets_response_data_and_notifies_child_snip721_of_migration() -> StdResult<()> {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

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
        let snip721_code_hash = child_snip721_code_hash();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            snip721_code_hash: snip721_code_hash.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let env = custom_mock_env_0();
        instantiate(
            deps.as_mut(),
            env.clone(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        )?;
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            custom_mock_env_0(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )?;

        let migrate_to_0 = ContractInfo {
            address: Addr::unchecked("new_address"),
            code_hash: "code_hash".to_string(),
        };

        let res = migrate(
            deps.as_mut(),
            admin_permit,
            &migrate_to_0.address.clone(),
            migrate_to_0.code_hash.as_str(),
        )?;

        let data: InstantiateByMigrationReplyDataMsg = from_binary(&res.data.clone().unwrap())?;

        let expected_data = valid_execute_migrate_reply(
            env.clone(),
            &prices,
            &purchasable_metadata,
            &admin_info.sender,
            &admin_permit.clone(),
            &snip721_code_hash,
            &Addr::unchecked(child_snip721_address.clone()),
            &MIGRATED_TO.load(deps.as_ref().storage)?.migration_secret,
        );
        assert_eq!(expected_data, data);

        assert_eq!(1, res.messages.len());
        assert_eq!(0, res.messages[0].id);
        assert_eq!(ReplyOn::Never, res.messages[0].reply_on);

        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[0].msg,
            &ContractInfo {
                address: Addr::unchecked(child_snip721_address),
                code_hash: snip721_code_hash,
            },
            &migrate_to_0,
        );
        Ok(())
    }

    #[test]
    fn on_migration_complete_contracts_registered_for_notification_are_notified() -> StdResult<()> {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

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
        let snip721_code_hash = child_snip721_address();

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            snip721_code_hash: snip721_code_hash.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let env = custom_mock_env_0();
        instantiate(
            deps.as_mut(),
            env.clone(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        )?;
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            custom_mock_env_0(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )?;
        // Force into Running mode for test
        CONTRACT_MODE.save(deps.as_mut().storage, &ContractMode::Running)?;

        let migrate_to_0 = ContractInfo {
            address: Addr::unchecked("new_address"),
            code_hash: "code_hash".to_string(),
        };

        let contracts_to_notify = vec![
            ContractInfo {
                address: Addr::unchecked("notify_0_address"),
                code_hash: "notify_0_code_hash".to_string(),
            },
            ContractInfo {
                address: Addr::unchecked("notify_1_address"),
                code_hash: "notify_1_code_hash".to_string(),
            },
        ];
        register_to_notify_on_migration_complete(
            deps.as_mut(),
            ContractMode::Running,
            contracts_to_notify[0].address.to_string(),
            contracts_to_notify[0].code_hash.clone(),
        )?;
        register_to_notify_on_migration_complete(
            deps.as_mut(),
            ContractMode::Running,
            contracts_to_notify[1].address.to_string(),
            contracts_to_notify[1].code_hash.clone(),
        )?;

        let res = migrate(
            deps.as_mut(),
            admin_permit,
            &migrate_to_0.address,
            migrate_to_0.code_hash.as_str(),
        )?;

        assert_eq!(3, res.messages.len());
        for sub_msg in &res.messages {
            assert_eq!(0, sub_msg.id);
            assert_eq!(ReplyOn::Never, sub_msg.reply_on);
        }

        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[0].msg,
            &ContractInfo {
                address: Addr::unchecked(child_snip721_address),
                code_hash: snip721_code_hash,
            },
            &migrate_to_0.clone(),
        );
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[1].msg,
            &contracts_to_notify[0].clone(),
            &migrate_to_0.clone(),
        );
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[2].msg,
            &contracts_to_notify[1].clone(),
            &migrate_to_0.clone(),
        );

        Ok(())
    }

    #[test]
    fn migrate_twice_fails() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        )
        .unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            custom_mock_env_0(),
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )
        .unwrap();

        let migrate_to_addr_0 = Addr::unchecked("new_address");
        let migrate_to_code_hash_0 = "code_hash";

        let migrate_0_result = migrate(
            deps.as_mut(),
            admin_permit,
            &migrate_to_addr_0,
            migrate_to_code_hash_0,
        );
        assert_eq!(
            true,
            migrate_0_result.is_ok(),
            "{:?}",
            migrate_0_result.unwrap_err()
        );

        let migrate_to_addr_1 = Addr::unchecked("new_address_1");
        let migrate_to_code_hash_1 = "code_hash_1";
        let migrate_1_result = migrate(
            deps.as_mut(),
            admin_permit,
            &migrate_to_addr_1,
            migrate_to_code_hash_1,
        );
        assert_eq!(false, migrate_1_result.is_ok());
        assert_eq!(
            build_operation_unavailable_error(&ContractMode::MigratedOut, None),
            migrate_1_result.err().unwrap(),
        );
    }

    #[test]
    fn register_to_notify_on_migration_complete_fails_when_in_invalid_contract_modes(
    ) -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        let admin_raw = deps.api.addr_canonicalize(admin_info.sender.as_str())?;
        ADMIN.save(deps.as_mut().storage, &admin_raw)?;
        let exec_purchase_msg =
            ExecuteMsg::Migrate(MigratableExecuteMsg::SubscribeToMigrationCompleteEvent {
                address: "".to_string(),
                code_hash: "".to_string(),
            });
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
            .collect();
        for invalid_mode in invalid_modes {
            CONTRACT_MODE.save(deps.as_mut().storage, &invalid_mode)?;
            let res = execute(
                deps.as_mut(),
                mock_env(),
                admin_info.clone(),
                exec_purchase_msg.clone(),
            );
            assert_eq!(
                build_operation_unavailable_error(&invalid_mode, None),
                res.err().unwrap(),
            );
        }
        Ok(())
    }

    #[test]
    fn migrate_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        let admin_raw = deps.api.addr_canonicalize(admin_info.sender.as_str())?;
        ADMIN.save(deps.as_mut().storage, &admin_raw)?;
        let exec_purchase_msg = ExecuteMsg::Migrate(MigratableExecuteMsg::Migrate {
            admin_permit: get_admin_permit(),
            migrate_to: MigrateTo {
                address: Addr::unchecked(""),
                code_hash: "".to_string(),
                entropy: "".to_string(),
            },
        });
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
            .collect();
        for invalid_mode in invalid_modes {
            CONTRACT_MODE.save(deps.as_mut().storage, &invalid_mode)?;
            let res = execute(
                deps.as_mut(),
                mock_env(),
                admin_info.clone(),
                exec_purchase_msg.clone(),
            );
            assert_eq!(
                build_operation_unavailable_error(&invalid_mode, None),
                res.err().unwrap(),
            );
        }
        Ok(())
    }

    #[test]
    fn migration_complete_notification_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        let admin_raw = deps.api.addr_canonicalize(admin_info.sender.as_str())?;
        ADMIN.save(deps.as_mut().storage, &admin_raw)?;
        let exec_purchase_msg = ExecuteMsg::MigrateListener(
            MigrationListenerExecuteMsg::MigrationCompleteNotification {
                to: ContractInfo {
                    address: Addr::unchecked(""),
                    code_hash: "".to_string(),
                },
                data: None,
            },
        );
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
            .collect();
        for invalid_mode in invalid_modes {
            CONTRACT_MODE.save(deps.as_mut().storage, &invalid_mode)?;
            let res = execute(
                deps.as_mut(),
                mock_env(),
                admin_info.clone(),
                exec_purchase_msg.clone(),
            );
            assert_eq!(
                build_operation_unavailable_error(&invalid_mode, None),
                res.err().unwrap(),
            );
        }
        Ok(())
    }

    #[test]
    fn register_to_notify_on_migration_complete_saves_contract() -> StdResult<()> {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAndChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        )?;

        let receiver = ContractInfo {
            address: Addr::unchecked("addr"),
            code_hash: "code_hash".to_string(),
        };
        execute(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            ExecuteMsg::Migrate(MigratableExecuteMsg::SubscribeToMigrationCompleteEvent {
                address: receiver.address.to_string(),
                code_hash: receiver.code_hash.to_string(),
            }),
        )?;

        let saved_contract = MIGRATION_COMPLETE_EVENT_SUBSCRIBERS.load(deps.as_ref().storage)?;
        assert_eq!(
            vec![canonicalize(deps.as_ref().api, &receiver)?],
            saved_contract
        );
        Ok(())
    }
}
