#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Binary, BlockInfo, Coin, ContractInfo, Deps, DepsMut, Env, from_binary, MessageInfo, Response, StdError, StdResult, Timestamp, TransactionInfo, Uint128};
    use cosmwasm_std::testing::{mock_dependencies, mock_info};
    use secret_toolkit::permit::{Permit, PermitParams, PermitSignature, PubKey, TokenPermissions, validate};
    use snip721_reference_impl::state::load;
    use snip721_reference_impl::token::Metadata;

    use migration::msg_types::{MigrateFrom, MigrateTo};
    use migration::state::{ContractMode, MIGRATED_TO_KEY, MigratedFrom};

    use crate::contract::{execute, instantiate, reply};
    use crate::msg::{CodeInfo, DealerState, ExecuteMsg, InstantiateByMigrationReplyDataMsg, InstantiateMsg, InstantiateSelfAnChildSnip721Msg};
    use crate::state::{CONTRACT_MODE_KEY, PurchasableMetadata};
    use crate::test_utils::test_utils::{child_snip721_address, successful_child_snip721_instantiate_reply};

    const CONTRACT_ADDRESS: &str = "secret1rf03820fp8gngzg2w02vd30ns78qkc8rg8dxaq";

    fn custom_mock_env() -> Env {
        Env {
            block: BlockInfo {
                height: 12_345,
                time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                chain_id: "cosmos-testnet-14002".to_string(),
            },
            transaction: Some(TransactionInfo { index: 3 }),
            contract: ContractInfo {
                address: Addr::unchecked(CONTRACT_ADDRESS),
                code_hash: "".to_string(),
            },
        }
    }

    fn get_admin_permit() -> Permit {
        Permit {
            params: PermitParams {
                allowed_tokens: vec![CONTRACT_ADDRESS.to_string()],
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
            CONTRACT_ADDRESS.to_string(),
            Some("secret"),
        )
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
            custom_mock_env(),
            MessageInfo {
                sender: Addr::unchecked(migration_target_addr),
                funds: vec![],
            },
            set_view_key_msg,
        );
        res
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

        let instantiate_msg = InstantiateSelfAnChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAnChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), custom_mock_env(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();


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
        let child_snip721_code_info = CodeInfo { code_id: 10, code_hash: "test_code_hash".to_string() };

        let instantiate_msg = InstantiateSelfAnChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            snip721_code_info: child_snip721_code_info.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAnChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), custom_mock_env(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();


        let migrate_to_addr_0 = Addr::unchecked("new_address");
        let migrate_to_code_hash_0 = "code_hash";

        let res = migrate(deps.as_mut(), admin_permit, &migrate_to_addr_0, migrate_to_code_hash_0).unwrap();


        let data: InstantiateByMigrationReplyDataMsg = from_binary(&res.data.unwrap()).unwrap();

        let expected_data = InstantiateByMigrationReplyDataMsg {
            dealer_state: DealerState {
                prices,
                public_metadata: purchasable_metadata.public_metadata,
                private_metadata: purchasable_metadata.private_metadata,
                admin: Addr::unchecked(admin_addr.clone()),
                child_snip721_code_info,
                child_snip721_address: Addr::unchecked(child_snip721_address),
            },
            migrate_from: MigrateFrom {
                address: custom_mock_env().contract.address,
                code_hash: custom_mock_env().contract.code_hash,
                admin_permit: admin_permit.clone(),
            },
            secret: load::<MigratedFrom>(deps.as_ref().storage, MIGRATED_TO_KEY).unwrap().migration_secret,
        };
        assert_eq!(expected_data, data)
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

        let instantiate_msg = InstantiateSelfAnChildSnip721Msg {
            prices: prices.clone(),
            admin: Some(admin_info.sender.to_string()),
            ..InstantiateSelfAnChildSnip721Msg::default()
        };
        let _res = instantiate(
            deps.as_mut(),
            custom_mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg),
        ).unwrap();
        // fake a reply after successful instantiate of child snip721
        let child_snip721_address = child_snip721_address();
        reply(deps.as_mut(), custom_mock_env(), successful_child_snip721_instantiate_reply(child_snip721_address.as_str())).unwrap();


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
