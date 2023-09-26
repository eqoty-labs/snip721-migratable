#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_info};
    use cosmwasm_std::{
        from_binary, Addr, Api, Binary, BlockInfo, CanonicalAddr, Coin, ContractInfo, CosmosMsg,
        Deps, DepsMut, Empty, Env, ReplyOn, StdResult, Timestamp, TransactionInfo, WasmMsg,
    };
    use cw_migratable_contract_std::execute::add_migration_complete_event_subscriber;
    use cw_migratable_contract_std::msg::{MigratableExecuteMsg, MigrationListenerExecuteMsg};
    use cw_migratable_contract_std::state::{
        canonicalize, CanonicalContractInfo, MIGRATION_COMPLETE_EVENT_SUBSCRIBERS,
    };
    use secret_toolkit::permit::{
        validate, Permit, PermitParams, PermitSignature, PubKey, TokenPermissions,
    };
    use snip721_reference_impl::state::{save, Config, CONFIG_KEY};

    use crate::contract::{execute, instantiate, update_migrated_dependency};
    use crate::contract_migrate::migrate;
    use crate::msg::ExecuteMsg;
    use crate::test_utils::test_utils::instantiate_msg;

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
    fn register_on_migration_complete_notify_receiver_saves_contract() -> StdResult<()> {
        let mut deps = mock_dependencies();
        let admin_permit = &get_admin_permit();
        let admin_addr = get_secret_address(deps.as_ref(), admin_permit)?;
        let admin_info = mock_info(admin_addr.as_str(), &[]);

        let instantiate_msg = instantiate_msg(admin_info.clone());
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
            custom_mock_env_0(),
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

    #[test]
    fn on_migration_complete_notification_sets_submsgs_to_notify_other_registered_contracts(
    ) -> StdResult<()> {
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

        let contracts_to_notify = vec![
            CanonicalContractInfo {
                address: deps
                    .api
                    .addr_canonicalize(Addr::unchecked("notify_0_address").as_str())?,
                code_hash: "notify_0_code_hash".to_string(),
            },
            CanonicalContractInfo {
                address: deps
                    .api
                    .addr_canonicalize(Addr::unchecked("notify_1_address").as_str())?,
                code_hash: "notify_1_code_hash".to_string(),
            },
        ];
        MIGRATION_COMPLETE_EVENT_SUBSCRIBERS.save(deps.as_mut().storage, &contracts_to_notify)?;

        let env_0_migrated = custom_mock_env_0_migrated();

        let res = migrate(deps.as_mut(), env_0_migrated.clone(), Empty::default())?;

        assert_eq!(2, res.messages.len());
        for sub_msg in &res.messages {
            assert_eq!(0, sub_msg.id);
            assert_eq!(ReplyOn::Never, sub_msg.reply_on);
        }
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[0].msg,
            &contracts_to_notify[0].humanize(deps.as_ref().api)?,
            &env_0_migrated.contract,
        );
        assert_is_migration_complete_notification_msg_to_contract(
            &res.messages[1].msg,
            &contracts_to_notify[1].humanize(deps.as_ref().api)?,
            &env_0_migrated.contract,
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
    fn update_migrated_subscriber_succeeds() -> StdResult<()> {
        let mut deps = mock_dependencies();
        save_a_config(deps.as_mut());
        let to = ContractInfo {
            address: Addr::unchecked("to_addr"),
            code_hash: "to_code_hash".to_string(),
        };
        let migrated_from_info = mock_info("migrated_from_addr", &[]);
        let migrated_from_code_hash = "migrated_from_code_hash".to_string();
        let raw_from_address = deps
            .as_ref()
            .api
            .addr_canonicalize(migrated_from_info.sender.as_str())?;
        add_migration_complete_event_subscriber(
            deps.as_mut().storage,
            &raw_from_address,
            &migrated_from_code_hash,
        )?;

        update_migrated_dependency(deps.as_mut(), migrated_from_info.clone(), to.clone())?;
        let stored_migration_complete_event_subscribers =
            MIGRATION_COMPLETE_EVENT_SUBSCRIBERS.load(deps.as_ref().storage)?;
        assert_eq!(
            vec![canonicalize(deps.as_ref().api, &to)?],
            stored_migration_complete_event_subscribers
        );
        Ok(())
    }
}
