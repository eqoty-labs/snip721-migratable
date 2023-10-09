#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{
        from_binary, Addr, Binary, BlockInfo, Coin, ContractInfo, CosmosMsg, Deps, Empty, Env,
        ReplyOn, StdResult, Timestamp, TransactionInfo, Uint128, WasmMsg,
    };
    use cw_migratable_contract_std::execute::register_to_notify_on_migration_complete;
    use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
    use cw_migratable_contract_std::state::{canonicalize, MIGRATION_COMPLETE_EVENT_SUBSCRIBERS};
    use secret_toolkit::permit::{
        validate, Permit, PermitParams, PermitSignature, PubKey, TokenPermissions,
    };
    use secret_toolkit::serialization::{Json, Serde};
    use snip721_reference_impl::token::Metadata;

    use crate::contract::{execute, instantiate, reply};
    use crate::contract_migrate::migrate;
    use crate::msg::{ExecuteMsg, InstantiateMsg};
    use crate::state::PurchasableMetadata;
    use crate::test_utils::test_utils::{
        child_snip721_address, child_snip721_code_hash, successful_child_snip721_instantiate_reply,
    };

    const CONTRACT_ADDRESS_0: &str = "secret1rf03820fp8gngzg2w02vd30ns78qkc8rg8dxaq";
    const _CONTRACT_ADDRESS_1: &str = "secret18eezxhys9jwku67cm4w84xhnzt4xjj772twz9k";

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
            transaction: Some(TransactionInfo {
                index: 3,
                hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            }),
            contract: ContractInfo {
                address: Addr::unchecked(CONTRACT_ADDRESS_0),
                code_hash: "code_hash_0".to_string(),
            },
        }
    }

    fn custom_mock_env_0_migrated() -> Env {
        Env {
            block: BlockInfo {
                height: 12_345,
                time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                chain_id: "cosmos-testnet-14002".to_string(),
                random: Some(Binary::from_base64("dGhlIGNha2UgaXMgYSBsaWU=").unwrap()),
            },
            transaction: Some(TransactionInfo {
                index: 3,
                hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            }),
            contract: ContractInfo {
                address: Addr::unchecked(CONTRACT_ADDRESS_0),
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
    fn migrate_notifies_child_snip721_of_migration() -> StdResult<()> {
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

        let instantiate_msg = InstantiateMsg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            snip721_code_hash: snip721_code_hash.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateMsg::default()
        };
        let env_0 = custom_mock_env_0();
        instantiate(
            deps.as_mut(),
            env_0.clone(),
            admin_info.clone(),
            instantiate_msg,
        )?;
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            env_0,
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
        )?;

        let env_0_migrated = custom_mock_env_0_migrated();

        let res = migrate(deps.as_mut(), env_0_migrated.clone(), Empty::default())?;

        assert_eq!(1, res.messages.len());
        assert_eq!(0, res.messages[0].id);
        assert_eq!(ReplyOn::Never, res.messages[0].reply_on);

        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[0].msg,
            &ContractInfo {
                address: Addr::unchecked(child_snip721_address),
                code_hash: snip721_code_hash,
            },
            &env_0_migrated.contract,
        );
        Ok(())
    }

    #[test]
    fn on_migration_complete_contracts_registered_for_notification_are_notified_including_child_snip721(
    ) -> StdResult<()> {
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

        let instantiate_msg = InstantiateMsg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            snip721_code_hash: snip721_code_hash.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateMsg::default()
        };
        let env = custom_mock_env_0();
        instantiate(
            deps.as_mut(),
            env.clone(),
            admin_info.clone(),
            instantiate_msg,
        )?;
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(
            deps.as_mut(),
            env,
            successful_child_snip721_instantiate_reply(child_snip721_address.as_str()),
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
        register_to_notify_on_migration_complete(
            deps.as_mut(),
            contracts_to_notify[0].address.to_string(),
            contracts_to_notify[0].code_hash.clone(),
        )?;
        register_to_notify_on_migration_complete(
            deps.as_mut(),
            contracts_to_notify[1].address.to_string(),
            contracts_to_notify[1].code_hash.clone(),
        )?;

        let env_0_migrated = custom_mock_env_0_migrated();

        let res = migrate(deps.as_mut(), env_0_migrated.clone(), Empty::default())?;

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
            &env_0_migrated.contract,
        );
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[1].msg,
            &contracts_to_notify[0].clone(),
            &env_0_migrated.contract,
        );
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[2].msg,
            &contracts_to_notify[1].clone(),
            &env_0_migrated.contract,
        );

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

        let instantiate_msg = InstantiateMsg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateMsg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env_0(),
            admin_info.clone(),
            instantiate_msg,
        )?;

        let receiver = ContractInfo {
            address: Addr::unchecked("addr"),
            code_hash: "code_hash".to_string(),
        };
        execute(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            Binary::from(
                Json::serialize(&ExecuteMsg::Migrate(
                    MigratableExecuteMsg::SubscribeToMigrationCompleteEvent {
                        address: receiver.address.to_string(),
                        code_hash: receiver.code_hash.to_string(),
                    },
                ))
                .unwrap(),
            ),
        )?;

        let saved_contract = MIGRATION_COMPLETE_EVENT_SUBSCRIBERS.load(deps.as_ref().storage)?;
        assert_eq!(
            vec![canonicalize(deps.as_ref().api, &receiver)?],
            saved_contract
        );
        Ok(())
    }
}
