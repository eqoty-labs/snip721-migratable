#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CanonicalAddr, Coin, ContractInfo, CosmosMsg, Deps, DepsMut, Env, from_binary, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsgResponse, SubMsgResult, Timestamp, to_binary, TransactionInfo, Uint128, WasmMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_info};
    use secret_toolkit::permit::{Permit, PermitParams, PermitSignature, PubKey, TokenPermissions, validate};
    use snip721_reference_impl::state::{load, may_load};
    use snip721_reference_impl::token::Metadata;

    use migration::msg_types::{MigrateFrom, MigrateTo};
    use migration::state::{ContractMode, MIGRATED_FROM_KEY, MIGRATED_TO_KEY, MigratedFrom};

    use crate::contract::{execute, instantiate, reply};
    use crate::msg::{CodeInfo, DealerState, ExecuteMsg, InstantiateByMigrationMsg, InstantiateByMigrationReplyDataMsg, InstantiateMsg, InstantiateSelfAndChildSnip721Msg};
    use crate::state::{ADMIN_KEY, CHILD_SNIP721_ADDRESS_KEY, CHILD_SNIP721_CODE_INFO_KEY, CONTRACT_MODE_KEY, PURCHASABLE_METADATA_KEY, PurchasableMetadata, PURCHASE_PRICES_KEY};
    use crate::test_utils::test_utils::{child_snip721_address, successful_child_snip721_instantiate_reply};

    const CONTRACT_ADDRESS_0: &str = "secret1rf03820fp8gngzg2w02vd30ns78qkc8rg8dxaq";
    const CONTRACT_ADDRESS_1: &str = "secret18eezxhys9jwku67cm4w84xhnzt4xjj772twz9k";

    fn custom_mock_env_0() -> Env {
        Env {
            block: BlockInfo {
                height: 12_345,
                time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                chain_id: "cosmos-testnet-14002".to_string(),
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
        child_snip721_code_info: &CodeInfo,
        child_snip721_address: &Addr,
        secret: &Binary,
    ) -> InstantiateByMigrationReplyDataMsg {
        InstantiateByMigrationReplyDataMsg {
            dealer_state: DealerState {
                prices: prices.clone(),
                public_metadata: purchasable_metadata.public_metadata.clone(),
                private_metadata: purchasable_metadata.private_metadata.clone(),
                admin: admin.clone(),
                child_snip721_code_info: child_snip721_code_info.clone(),
                child_snip721_address: child_snip721_address.clone(),
            },
            migrate_from: MigrateFrom {
                address: env.contract.address,
                code_hash: env.contract.code_hash,
                admin_permit: admin_permit.clone(),
            },
            secret: secret.clone(),
        }
    }

    pub fn migrate(
        deps: DepsMut,
        admin_permit: &Permit,
        migration_target_addr: &Addr,
        migration_target_code_hash: &str,
    ) -> StdResult<Response> {
        let set_view_key_msg =
            ExecuteMsg::Migrate {
                admin_permit: admin_permit.clone(),
                migrate_to: MigrateTo {
                    address: migration_target_addr.clone(),
                    code_hash: migration_target_code_hash.to_string(),
                    entropy: "magnets, how do they work?".to_string(),
                },
            };
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

    #[test]
    fn instantiate_by_migration_correctly_migrates_dealer_state() {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
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
        assert_ne!(migrate_to_env.contract.address.clone(), migrate_from_env.contract.address.clone());
        assert_ne!(migrate_to_env.contract.code_hash.clone(), migrate_from_env.contract.code_hash.clone());

        let res = instantiate(
            deps.as_mut(),
            migrate_to_env.clone(),
            admin_info.clone(),
            InstantiateMsg::Migrate(instantiate_msg.clone()),
        ).unwrap();
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
                    contract_addr, code_hash, msg, funds
                } => {
                    assert_eq!(&migrate_from.address, contract_addr);
                    assert_eq!(&migrate_from.code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    let execute_msg: ExecuteMsg = from_binary(msg).unwrap();
                    let expected_execute_msg = ExecuteMsg::Migrate {
                        admin_permit: admin_permit.clone(),
                        migrate_to: MigrateTo {
                            address: migrate_to_env.contract.address.clone(),
                            code_hash: migrate_to_env.contract.code_hash.clone(),
                            entropy: instantiate_msg.entropy,
                        },
                    };
                    assert_eq!(expected_execute_msg, execute_msg);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }


        // none of the dealer state should be set before reply is called
        let saved_prices: Option<Vec<Coin>> = may_load(deps.as_ref().storage, PURCHASE_PRICES_KEY).unwrap();
        assert_eq!(None, saved_prices);
        let saved_purchasable_metadata: Option<PurchasableMetadata> = may_load(deps.as_ref().storage, PURCHASABLE_METADATA_KEY).unwrap();
        assert_eq!(None, saved_purchasable_metadata);
        let saved_admin: Option<CanonicalAddr> = may_load(deps.as_ref().storage, ADMIN_KEY).unwrap();
        assert_eq!(None, saved_admin);
        let saved_child_snip721_code_info: Option<CodeInfo> = may_load(deps.as_ref().storage, CHILD_SNIP721_CODE_INFO_KEY).unwrap();
        assert_eq!(None, saved_child_snip721_code_info);
        let saved_child_snip721_address: Option<CanonicalAddr> = may_load(deps.as_ref().storage, CHILD_SNIP721_ADDRESS_KEY).unwrap();
        if let Some(some_saved_child_snip721_address) = saved_child_snip721_address {
            assert_eq!(None, Some(deps.api.addr_humanize(&some_saved_child_snip721_address).unwrap()));
        }

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
        let migrated_snip721_code_info = CodeInfo { code_id: 10, code_hash: "test_code_hash".to_string() };
        let migrated_child_snip721_address = child_snip721_address();
        let migrate_from_secret = &Binary::from(b"some_secret");
        let exec_migrate_reply_msg_data = to_binary(&valid_execute_migrate_reply(
            migrate_from_env.clone(),
            &migrated_prices,
            &migrated_purchasable_metadata,
            &admin_info.sender,
            &admin_permit.clone(),
            &migrated_snip721_code_info,
            &Addr::unchecked(migrated_child_snip721_address.clone()),
            migrate_from_secret,
        )).unwrap();

        // fake a reply after successful execution of migrate from old version of dealer
        reply(deps.as_mut(), migrate_to_env.clone(),
              Reply {
                  id: 2u64,
                  result: SubMsgResult::Ok(SubMsgResponse {
                      data: Some(exec_migrate_reply_msg_data),
                      events: Vec::new(),
                  }),
              },
        ).unwrap();

        // dealer state should be saved using data from migration
        let saved_prices: Vec<Coin> = load(deps.as_ref().storage, PURCHASE_PRICES_KEY).unwrap();
        assert_eq!(migrated_prices, saved_prices);
        let saved_purchasable_metadata: PurchasableMetadata = load(deps.as_ref().storage, PURCHASABLE_METADATA_KEY).unwrap();
        assert_eq!(migrated_purchasable_metadata, saved_purchasable_metadata);
        let saved_admin: CanonicalAddr = load(deps.as_ref().storage, ADMIN_KEY).unwrap();
        assert_eq!(deps.api.addr_canonicalize(admin_info.sender.as_str()).unwrap(), saved_admin);
        let saved_child_snip721_code_info: CodeInfo = load(deps.as_ref().storage, CHILD_SNIP721_CODE_INFO_KEY).unwrap();
        assert_eq!(migrated_snip721_code_info, saved_child_snip721_code_info);
        let saved_child_snip721_address: CanonicalAddr = load(deps.as_ref().storage, CHILD_SNIP721_ADDRESS_KEY).unwrap();
        assert_eq!(migrated_child_snip721_address, deps.api.addr_humanize(&saved_child_snip721_address).unwrap());
        let expected_migrated_from = MigratedFrom {
            contract: ContractInfo {
                address: migrate_from_env.contract.address.clone(),
                code_hash: migrate_from_env.contract.code_hash.clone(),
            },
            migration_secret: migrate_from_secret.clone(),
        };
        let migrated_from: MigratedFrom = load(deps.as_ref().storage, MIGRATED_FROM_KEY).unwrap();
        assert_eq!(expected_migrated_from, migrated_from);
    }

    #[test]
    fn migrate_updates_contract_mode_and_migrated_to_state() {
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
        ).unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), custom_mock_env_0(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();


        let migrate_to_addr_0 = Addr::unchecked("new_address");
        let migrate_to_code_hash_0 = "code_hash";

        migrate(deps.as_mut(), admin_permit, &migrate_to_addr_0, migrate_to_code_hash_0).unwrap();

        assert_eq!(ContractMode::MigratedOut, load::<ContractMode>(deps.as_ref().storage, CONTRACT_MODE_KEY).unwrap());
        assert_eq!(
            ContractInfo {
                address: migrate_to_addr_0,
                code_hash: migrate_to_code_hash_0.to_string(),
            },
            load::<ContractInfo>(deps.as_ref().storage, MIGRATED_TO_KEY).unwrap()
        );
    }

    #[test]
    fn migrate_sets_response_data() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
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
        let snip721_code_info = CodeInfo { code_id: 10, code_hash: "test_code_hash".to_string() };

        let instantiate_msg = InstantiateSelfAndChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            snip721_code_info: snip721_code_info.clone(),
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
        ).unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), custom_mock_env_0(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();


        let migrate_to_addr_0 = Addr::unchecked("new_address");
        let migrate_to_code_hash_0 = "code_hash";

        let res = migrate(deps.as_mut(), admin_permit, &migrate_to_addr_0, migrate_to_code_hash_0).unwrap();


        let data: InstantiateByMigrationReplyDataMsg = from_binary(&res.data.clone().unwrap()).unwrap();

        let expected_data = valid_execute_migrate_reply(
            env.clone(),
            &prices,
            &purchasable_metadata,
            &admin_info.sender,
            &admin_permit.clone(),
            &snip721_code_info,
            &Addr::unchecked(child_snip721_address.clone()),
            &load::<MigratedFrom>(deps.as_ref().storage, MIGRATED_TO_KEY).unwrap().migration_secret,
        );
        assert_eq!(expected_data, data);
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
        ).unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), custom_mock_env_0(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();


        let migrate_to_addr_0 = Addr::unchecked("new_address");
        let migrate_to_code_hash_0 = "code_hash";

        let migrate_0_result = migrate(deps.as_mut(), admin_permit, &migrate_to_addr_0, migrate_to_code_hash_0);
        assert_eq!(true, migrate_0_result.is_ok(), "{:?}", migrate_0_result.unwrap_err());

        let migrate_to_addr_1 = Addr::unchecked("new_address_1");
        let migrate_to_code_hash_1 = "code_hash_1";
        let migrate_1_result = migrate(deps.as_mut(), admin_permit, &migrate_to_addr_1, migrate_to_code_hash_1);
        assert_eq!(false, migrate_1_result.is_ok());
        assert_eq!(
            migrate_1_result.err().unwrap(),
            StdError::generic_err(format!(
                "This contract has been migrated to {:?}. No further state changes are allowed!",
                migrate_to_addr_0,
            ), )
        );
    }
}