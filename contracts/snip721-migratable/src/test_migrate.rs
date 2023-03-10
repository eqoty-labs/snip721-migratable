#[cfg(test)]
mod tests {
    use cosmwasm_contract_migratable_std::execute::build_operation_unavailable_error;
    use cosmwasm_contract_migratable_std::msg::{
        MigratableExecuteMsg, MigrationListenerExecuteMsg,
    };
    use cosmwasm_contract_migratable_std::msg_types::{MigrateFrom, MigrateTo};
    use cosmwasm_contract_migratable_std::state::{CONTRACT_MODE_KEY, ContractMode, MIGRATED_FROM_KEY, MIGRATED_TO_KEY, MigratedFromState, MigratedToState, NOTIFY_ON_MIGRATION_COMPLETE_KEY};
    use cosmwasm_std::{
        Addr, Api, Binary, BlockInfo, CanonicalAddr, Coin, ContractInfo, CosmosMsg, Deps,
        DepsMut, Env, from_binary, MessageInfo, Reply, ReplyOn, Response, StdResult, SubMsgResponse,
        SubMsgResult, Timestamp, to_binary, TransactionInfo, Uint128, WasmMsg,
    };
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use secret_toolkit::permit::{
        Permit, PermitParams, PermitSignature, PubKey, TokenPermissions, validate,
    };
    use snip721_reference_impl::msg::BatchNftDossierElement;
    use snip721_reference_impl::msg::ExecuteMsg as Snip721ExecuteMsg;
    use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;
    use snip721_reference_impl::state::{Config, CONFIG_KEY, load, may_load, MINTERS_KEY, save};
    use snip721_reference_impl::token::Metadata;
    use strum::IntoEnumIterator;

    use crate::contract::{
        execute, instantiate, on_migration_complete, query, reply, update_migrated_minter,
    };
    use crate::msg::{
        ExecuteMsg, ExecuteMsgExt, InstantiateByMigrationReplyDataMsg, QueryAnswer, QueryMsg,
    };
    use crate::msg::QueryAnswer::MigrationBatchNftDossier;
    use crate::msg::QueryMsgExt::ExportMigrationData;
    use crate::state::{MIGRATE_IN_TOKENS_PROGRESS_KEY, MigrateInTokensProgress};
    use crate::test_utils::test_utils::instantiate_msg;

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

    pub fn build_mint_msg(
        recipient: String,
        public_metadata: Option<Metadata>,
        private_metadata: Option<Metadata>,
    ) -> ExecuteMsg {
        ExecuteMsg::Base(Snip721ExecuteMsg::MintNft {
            token_id: None,
            owner: Some(recipient),
            public_metadata,
            private_metadata,
            serial_number: None,
            royalty_info: None,
            transferable: None,
            memo: None,
            padding: None,
        })
    }

    pub fn set_viewing_key(deps: DepsMut, viewing_key: String, message_info: MessageInfo) {
        let set_view_key_msg =
            ExecuteMsg::Base(snip721_reference_impl::msg::ExecuteMsg::SetViewingKey {
                key: viewing_key.clone(),
                padding: None,
            });
        let res = execute(
            deps,
            custom_mock_env_0(),
            message_info.clone(),
            set_view_key_msg,
        );
        assert!(res.is_ok(), "execute failed: {}", res.err().unwrap());
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

    pub fn get_tokens(deps: Deps, viewing_key: String, message_info: MessageInfo) -> Vec<String> {
        let query_msg = QueryMsg::Base(snip721_reference_impl::msg::QueryMsg::Tokens {
            owner: message_info.sender.to_string(),
            viewer: None,
            viewing_key: Some(viewing_key.clone()),
            start_after: None,
            limit: None,
        });
        let query_res = query(deps, custom_mock_env_0(), query_msg);

        assert!(
            query_res.is_ok(),
            "query failed: {}",
            query_res.err().unwrap()
        );
        let query_answer: StdResult<snip721_reference_impl::msg::QueryAnswer> =
            from_binary(&query_res.unwrap());
        if query_answer.is_ok() {
            return match query_answer.unwrap() {
                snip721_reference_impl::msg::QueryAnswer::TokenList { tokens } => tokens,
                _ => panic!("unexpected"),
            };
        } else {
            panic!("{}", query_answer.unwrap_err())
        }
    }

    pub fn export_migration_data(
        deps: Deps,
        start_index: Option<u32>,
        max_count: Option<u32>,
        secret: Binary,
    ) -> Vec<BatchNftDossierElement> {
        let query_msg = QueryMsg::Ext(ExportMigrationData {
            start_index,
            max_count,
            secret,
        });
        let query_res = query(deps, custom_mock_env_0(), query_msg);

        assert!(
            query_res.is_ok(),
            "query failed: {}",
            query_res.err().unwrap()
        );
        let query_answer: StdResult<QueryAnswer> = from_binary(&query_res.unwrap());
        if query_answer.is_ok() {
            return match query_answer.unwrap() {
                MigrationBatchNftDossier {
                    last_mint_index: _,
                    nft_dossiers,
                } => nft_dossiers,
            };
        } else {
            panic!("{}", query_answer.unwrap_err())
        }
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
    fn instantiate_with_migrated_config_saves_config() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
        let env_0 = custom_mock_env_0();
        let env_1 = custom_mock_env_1();
        let snip721_dealer_to_notify = ContractInfo {
            address: Addr::unchecked("dealer_addr"),
            code_hash: "dealer_code_hash".to_string(),
        };
        let expected_mint_count = 3;
        let expected_secret = Binary::from(b"secret_to_migrate_data_in");
        let expected_minters = vec![deps
            .api
            .addr_canonicalize(snip721_dealer_to_notify.address.as_str())?];
        let instantiate_by_migration_reply_msg_data =
            to_binary(&InstantiateByMigrationReplyDataMsg {
                migrated_instantiate_msg: Snip721InstantiateMsg {
                    name: "migratable_snip721".to_string(),
                    admin: Some(admin_addr),
                    entropy: "".to_string(),
                    royalty_info: None,
                    config: None,
                    symbol: "".to_string(),
                    post_init_callback: None,
                },
                migrate_from: MigrateFrom {
                    address: env_0.contract.address.clone(),
                    code_hash: env_0.contract.code_hash.clone(),
                    admin_permit: admin_permit.clone(),
                },
                on_migration_complete_notify_receiver: Some(vec![snip721_dealer_to_notify.clone()]),
                minters: expected_minters.clone(),
                mint_count: expected_mint_count,
                secret: expected_secret.clone(),
            })
                .unwrap();
        let reply_msg = Reply {
            id: 1u64,
            result: SubMsgResult::Ok(SubMsgResponse {
                data: Some(instantiate_by_migration_reply_msg_data),
                events: Vec::new(),
            }),
        };
        reply(deps.as_mut(), env_1, reply_msg)?;

        let expected_migrated_from = MigratedFromState {
            contract: env_0.contract,
            migration_secret: expected_secret,
        };
        assert_eq!(
            expected_migrated_from,
            load::<MigratedFromState>(deps.as_ref().storage, MIGRATED_FROM_KEY)?
        );

        let expected_migrate_in_tokens_progress = MigrateInTokensProgress {
            migrate_in_mint_cnt: expected_mint_count,
            migrate_in_next_mint_index: 0,
        };
        assert_eq!(
            expected_migrate_in_tokens_progress,
            load::<MigrateInTokensProgress>(deps.as_ref().storage, MIGRATE_IN_TOKENS_PROGRESS_KEY)?
        );
        assert_eq!(
            ContractMode::MigrateDataIn,
            load::<ContractMode>(deps.as_ref().storage, CONTRACT_MODE_KEY)?
        );
        assert_eq!(
            vec![snip721_dealer_to_notify],
            load::<Vec<ContractInfo>>(deps.as_ref().storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY)?
        );
        assert_eq!(
            expected_minters,
            load::<Vec<CanonicalAddr>>(deps.as_ref().storage, MINTERS_KEY)?
        );

        Ok(())
    }

    #[test]
    fn instantiate_with_migrated_config_clears_response_data() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
        let env_0 = custom_mock_env_0();
        let env_1 = custom_mock_env_1();
        let snip721_dealer_to_notify = ContractInfo {
            address: Addr::unchecked("dealer_addr"),
            code_hash: "dealer_code_hash".to_string(),
        };
        let expected_mint_count = 3;
        let expected_secret = Binary::from(b"secret_to_migrate_data_in");
        let instantiate_by_migration_reply_msg_data =
            to_binary(&InstantiateByMigrationReplyDataMsg {
                migrated_instantiate_msg: Snip721InstantiateMsg {
                    name: "migratable_snip721".to_string(),
                    admin: Some(admin_addr),
                    entropy: "".to_string(),
                    royalty_info: None,
                    config: None,
                    symbol: "".to_string(),
                    post_init_callback: None,
                },
                migrate_from: MigrateFrom {
                    address: env_0.contract.address.clone(),
                    code_hash: env_0.contract.code_hash.clone(),
                    admin_permit: admin_permit.clone(),
                },
                on_migration_complete_notify_receiver: Some(vec![snip721_dealer_to_notify.clone()]),
                minters: vec![],
                mint_count: expected_mint_count,
                secret: expected_secret.clone(),
            })
                .unwrap();
        let reply_msg = Reply {
            id: 1u64,
            result: SubMsgResult::Ok(SubMsgResponse {
                data: Some(instantiate_by_migration_reply_msg_data),
                events: Vec::new(),
            }),
        };
        let res = reply(deps.as_mut(), env_1, reply_msg)?;
        assert_eq!(res.data, Some(Binary::from(b"")));
        Ok(())
    }

    #[test]
    fn migrate_twice_fails() {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
        let admin_info = mock_info(admin_addr.as_str(), &[]);
        let mint_recipient_info = mock_info("minty", &[]);

        let instantiate_msg = instantiate_msg(admin_info.clone());
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            instantiate_msg,
        )
            .unwrap();

        execute(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            build_mint_msg(mint_recipient_info.sender.to_string(), None, None).clone(),
        )
            .unwrap();

        let viewing_key = "key".to_string();
        set_viewing_key(
            deps.as_mut(),
            viewing_key.clone(),
            mint_recipient_info.clone(),
        );
        let tokens = get_tokens(
            deps.as_ref(),
            viewing_key.clone(),
            mint_recipient_info.clone(),
        );
        assert_eq!(tokens.len(), 1);

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
            build_operation_unavailable_error(&ContractMode::MigrateOutStarted, None),
            migrate_1_result.err().unwrap(),
        );
    }

    #[test]
    fn export_migration_data_three_tokens() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
        let admin_info = mock_info(admin_addr.as_str(), &[]);
        let mint_recipient_0_info = mock_info("minty_0", &prices.clone());
        let mint_recipient_1_info = mock_info("minty_1", &prices.clone());
        let mint_recipient_2_info = mock_info("minty_2", &prices.clone());

        let public_metadata = Some(Metadata {
            token_uri: Some("public_metadata_uri".to_string()),
            extension: None,
        });
        let private_metadata = Some(Metadata {
            token_uri: Some("private_metadata_uri".to_string()),
            extension: None,
        });
        let instantiate_msg = instantiate_msg(admin_info.clone());
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            instantiate_msg,
        )
            .unwrap();

        execute(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            build_mint_msg(
                mint_recipient_0_info.sender.to_string(),
                public_metadata.clone(),
                private_metadata.clone(),
            )
                .clone(),
        )
            .unwrap();
        execute(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            build_mint_msg(
                mint_recipient_1_info.sender.to_string(),
                public_metadata.clone(),
                private_metadata.clone(),
            )
                .clone(),
        )
            .unwrap();
        execute(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            build_mint_msg(
                mint_recipient_2_info.sender.to_string(),
                public_metadata.clone(),
                private_metadata.clone(),
            )
                .clone(),
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
            migrate_0_result.is_ok(),
            true,
            "{:?}",
            migrate_0_result.unwrap_err()
        );

        let migrate_data: InstantiateByMigrationReplyDataMsg =
            from_binary(&migrate_0_result.unwrap().data.unwrap()).unwrap();

        let secret: Binary = migrate_data.secret;

        let migration_data = export_migration_data(deps.as_ref(), Some(0), Some(3), secret);

        let first_token = migration_data[0].clone();
        assert_eq!("0", first_token.token_id.clone());
        assert_eq!(
            public_metadata.clone().unwrap(),
            first_token.public_metadata.unwrap()
        );
        assert_eq!(
            private_metadata.clone().unwrap(),
            first_token.private_metadata.unwrap()
        );
        assert_eq!(mint_recipient_0_info.sender, first_token.owner.unwrap());

        let second_token = migration_data[1].clone();
        assert_eq!("1", second_token.token_id.clone());
        assert_eq!(
            public_metadata.clone().unwrap(),
            second_token.public_metadata.unwrap()
        );
        assert_eq!(
            private_metadata.clone().unwrap(),
            second_token.private_metadata.unwrap()
        );
        assert_eq!(mint_recipient_1_info.sender, second_token.owner.unwrap());

        let third_token = migration_data[2].clone();
        assert_eq!("2", third_token.token_id.clone());
        assert_eq!(
            public_metadata.unwrap(),
            third_token.public_metadata.unwrap()
        );
        assert_eq!(
            private_metadata.unwrap(),
            third_token.private_metadata.unwrap()
        );
        assert_eq!(mint_recipient_2_info.sender, third_token.owner.unwrap());
    }

    #[test]
    fn register_on_migration_complete_notify_receiver_saves_contract() {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit).unwrap();
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = instantiate_msg(admin_info.clone());
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            instantiate_msg,
        )
            .unwrap();

        let receiver = ContractInfo {
            address: Addr::unchecked("addr"),
            code_hash: "code_hash".to_string(),
        };
        execute(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            ExecuteMsg::Migrate(MigratableExecuteMsg::RegisterToNotifyOnMigrationComplete {
                address: receiver.address.to_string(),
                code_hash: receiver.code_hash.to_string(),
            }),
        )
            .unwrap();

        let saved_contract: Vec<ContractInfo> =
            load(deps.as_ref().storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY).unwrap();
        assert_eq!(vec![receiver], saved_contract);
    }

    #[test]
    fn migrate_data_in_adds_message_to_notify_migrated_contract_of_completion() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = instantiate_msg(admin_info.clone());
        let env_0 = custom_mock_env_0();
        let _res = instantiate(
            deps.as_mut(),
            env_0.clone(),
            admin_info.clone(),
            instantiate_msg,
        )?;
        save(
            deps.as_mut().storage,
            CONTRACT_MODE_KEY,
            &ContractMode::MigrateDataIn,
        )?;
        let mock_secret = Binary::from(b"secret_to_migrate_data_in");
        let mock_migrated_from = MigratedFromState {
            contract: env_0.contract,
            migration_secret: mock_secret,
        };
        save(
            deps.as_mut().storage,
            MIGRATED_FROM_KEY,
            &mock_migrated_from,
        )?;
        let mock_migrate_in_tokens_progress = MigrateInTokensProgress {
            migrate_in_mint_cnt: 0,
            migrate_in_next_mint_index: 0,
        };
        save(
            deps.as_mut().storage,
            MIGRATE_IN_TOKENS_PROGRESS_KEY,
            &mock_migrate_in_tokens_progress,
        )?;

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
        save(
            deps.as_mut().storage,
            NOTIFY_ON_MIGRATION_COMPLETE_KEY,
            &contracts_to_notify,
        )?;

        let contracts_that_should_not_be_notified_yet =
            may_load::<Vec<ContractInfo>>(deps.as_ref().storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY)?
                .unwrap_or_default();
        assert_eq!(2, contracts_that_should_not_be_notified_yet.len());

        let res = execute(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            ExecuteMsg::Ext(ExecuteMsgExt::MigrateTokensIn {
                pages: None,
                page_size: None,
            }),
        )?;

        assert_eq!(1, res.messages.len());
        assert_eq!(0, res.messages[0].id);
        assert_eq!(ReplyOn::Never, res.messages[0].reply_on);
        assert!(matches!(
            res.messages[0].msg,
            CosmosMsg::Wasm(WasmMsg::Execute { .. })
        ));
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[0].msg,
            &mock_migrated_from.contract.clone(),
            &mock_migrated_from.contract,
        );

        Ok(())
    }

    #[test]
    fn on_migration_complete_notification_sets_submsgs_to_notify_other_registered_contracts() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = instantiate_msg(admin_info.clone());
        let env_0 = custom_mock_env_0();
        let _res = instantiate(
            deps.as_mut(),
            env_0.clone(),
            admin_info.clone(),
            instantiate_msg,
        )?;
        save(
            deps.as_mut().storage,
            CONTRACT_MODE_KEY,
            &ContractMode::MigrateOutStarted,
        )?;
        let migrated_to = ContractInfo {
            address: Addr::unchecked("new_address"),
            code_hash: "code_hash".to_string(),
        };

        save(
            deps.as_mut().storage,
            MIGRATED_TO_KEY,
            &MigratedToState { contract: migrated_to.clone(), migration_secret: Default::default() },
        )?;

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
        save(
            deps.as_mut().storage,
            NOTIFY_ON_MIGRATION_COMPLETE_KEY,
            &contracts_to_notify,
        )?;

        let migrated_to = ContractInfo {
            address: Addr::unchecked("new_address"),
            code_hash: "code_hash".to_string(),
        };
        let migrated_to_info = mock_info(migrated_to.address.as_str(), &[]);
        let res = execute(
            deps.as_mut(),
            custom_mock_env_0(),
            migrated_to_info.clone(),
            ExecuteMsg::MigrateListener(MigrationListenerExecuteMsg::MigrationCompleteNotification {
                to: migrated_to.clone(),
                data: None,
            }),
        )?;

        assert_eq!(2, res.messages.len());
        for sub_msg in &res.messages {
            assert_eq!(0, sub_msg.id);
            assert_eq!(ReplyOn::Never, sub_msg.reply_on);
        }
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[0].msg,
            &contracts_to_notify[0].clone(),
            &migrated_to.clone(),
        );
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[1].msg,
            &contracts_to_notify[1].clone(),
            &migrated_to,
        );

        Ok(())
    }

    fn save_a_config(deps: DepsMut) {
        save(
            deps.storage,
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
        )
            .unwrap();
    }

    #[test]
    fn migrate_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        save_a_config(deps.as_mut());
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
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
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
    fn perform_token_migration_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        save_a_config(deps.as_mut());
        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::MigrateTokensIn {
            pages: None,
            page_size: None,
        });
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::MigrateDataIn)
            .collect();
        for invalid_mode in invalid_modes {
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
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
    fn register_to_notify_on_migration_complete_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        save_a_config(deps.as_mut());
        let exec_purchase_msg =
            ExecuteMsg::Migrate(MigratableExecuteMsg::RegisterToNotifyOnMigrationComplete {
                address: "".to_string(),
                code_hash: "".to_string(),
            });
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
            .collect();
        for invalid_mode in invalid_modes {
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
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
    fn execute_snip721_base_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        save_a_config(deps.as_mut());
        let exec_purchase_msg =
            ExecuteMsg::Base(snip721_reference_impl::msg::ExecuteMsg::CreateViewingKey {
                entropy: "".to_string(),
                padding: None,
            });
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
            .collect();
        for invalid_mode in invalid_modes {
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
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
        save_a_config(deps.as_mut());
        let msg = ExecuteMsg::MigrateListener(
            MigrationListenerExecuteMsg::MigrationCompleteNotification {
                to: ContractInfo {
                    address: Addr::unchecked("from_addr"),
                    code_hash: "".to_string(),
                },
                data: None,
            },
        );
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running && m != &ContractMode::MigrateOutStarted)
            .collect();
        for invalid_mode in invalid_modes {
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
            let res = execute(deps.as_mut(), mock_env(), admin_info.clone(), msg.clone());
            assert_eq!(
                build_operation_unavailable_error(&invalid_mode, None),
                res.err().unwrap(),
            );
        }
        Ok(())
    }

    #[test]
    fn update_migrated_minter_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        save_a_config(deps.as_mut());
        let to = ContractInfo {
            address: Addr::unchecked("to_addr"),
            code_hash: "".to_string(),
        };
        let migrated_from_info = mock_info("migrated_from_addr", &[]);

        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::Running)
            .collect();
        for invalid_mode in invalid_modes {
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
            let expected_error = build_operation_unavailable_error(&invalid_mode, None);
            let res = update_migrated_minter(deps.as_mut(), migrated_from_info.clone(), invalid_mode, to.clone());
            assert_eq!(expected_error, res.err().unwrap(), );
        }
        Ok(())
    }

    #[test]
    fn on_migration_complete_fails_when_in_invalid_contract_modes() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_info = mock_info("admin", &[]);
        save_a_config(deps.as_mut());
        let invalid_modes: Vec<ContractMode> = ContractMode::iter()
            .filter(|m| m != &ContractMode::MigrateOutStarted)
            .collect();
        for invalid_mode in invalid_modes {
            save(deps.as_mut().storage, CONTRACT_MODE_KEY, &invalid_mode)?;
            let expected_error = build_operation_unavailable_error(&invalid_mode, None);
            let res = on_migration_complete(deps.as_mut(), admin_info.clone(), invalid_mode);
            assert_eq!(expected_error, res.err().unwrap(), );
        }
        Ok(())
    }
}
