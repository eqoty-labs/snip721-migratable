#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, from_binary, MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use snip721_reference_impl::msg::BatchNftDossierElement;
    use snip721_reference_impl::token::Metadata;

    use crate::contract::{execute, instantiate, query};
    use crate::msg::{ExecuteMsg, ExecuteMsgExt, InstantiateMsg, MigrationContractTargetExecuteMsg, QueryMsg};
    use crate::msg::QueryMsgExt::ExportMigrationData;

    pub fn instantiate_msg(prices: Vec<Coin>, public_metadata: Option<Metadata>, private_metadata: Option<Metadata>, admin_info: MessageInfo) -> InstantiateMsg {
        InstantiateMsg {
            prices: prices.clone(),
            public_metadata,
            private_metadata,
            admin: admin_info.sender.to_string(),
            entropy: "".to_string(),
            royalty_info: None,
        }
    }

    pub fn set_viewing_key(deps: DepsMut, viewing_key: String, message_info: MessageInfo) {
        let set_view_key_msg =
            ExecuteMsg::Base(snip721_reference_impl::msg::ExecuteMsg::SetViewingKey {
                key: viewing_key.clone(),
                padding: None,
            });
        let res = execute(deps, mock_env(), message_info.clone(), set_view_key_msg);
        assert!(res.is_ok(), "execute failed: {}", res.err().unwrap());
    }

    pub fn migrate(
        deps: DepsMut,
        admin_message_info: &MessageInfo,
        migration_target_addr: &str,
        migration_target_code_hash: &str,
    ) -> StdResult<Response> {
        let set_view_key_msg =
            ExecuteMsg::Ext(ExecuteMsgExt::Migrate {
                address: migration_target_addr.to_string(),
                code_hash: migration_target_code_hash.to_string(),
            });
        let res = execute(deps, mock_env(), admin_message_info.clone(), set_view_key_msg);
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
        let query_res = query(deps, mock_env(), query_msg);

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

    pub fn export_migration_data(deps: Deps, token_ids: Vec<String>, secret: Binary) -> Vec<BatchNftDossierElement> {
        let query_msg = QueryMsg::Ext(ExportMigrationData {
            token_ids,
            secret,
        });
        let query_res = query(deps, mock_env(), query_msg);

        assert!(
            query_res.is_ok(),
            "query failed: {}",
            query_res.err().unwrap()
        );
        let query_answer: StdResult<snip721_reference_impl::msg::QueryAnswer> =
            from_binary(&query_res.unwrap());
        if query_answer.is_ok() {
            return match query_answer.unwrap() {
                snip721_reference_impl::msg::QueryAnswer::BatchNftDossier { nft_dossiers } => nft_dossiers,
                _ => panic!("unexpected"),
            };
        } else {
            panic!("{}", query_answer.unwrap_err())
        }
    }

    #[test]
    fn migrate_twice_fails() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let admin_info = mock_info("creator", &[]);
        let minter_info = mock_info("minty", &prices.clone());
        let pay_to_addr = admin_info.sender.clone();

        let mut deps = mock_dependencies();

        let instantiate_msg = instantiate_msg(prices.clone(), None, None, admin_info.clone());
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        let exec_purchase_res = execute(
            deps.as_mut(),
            mock_env(),
            minter_info.clone(),
            exec_purchase_msg,
        ).unwrap();

        // there should be one message
        assert_eq!(exec_purchase_res.messages.len(), 1);
        // the message should be a Bank Send message
        println!("{:#?}", exec_purchase_res.messages[0].msg);
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

        let viewing_key = "key".to_string();
        set_viewing_key(deps.as_mut(), viewing_key.clone(), minter_info.clone());
        let tokens = get_tokens(deps.as_ref(), viewing_key.clone(), minter_info.clone());
        assert_eq!(tokens.len(), 1);

        let migrate_to_addr_0 = "new_address";
        let migrate_to_code_hash_0 = "code_hash";

        let migrate_0_result = migrate(deps.as_mut(), &admin_info, migrate_to_addr_0, migrate_to_code_hash_0);
        assert_eq!(true, migrate_0_result.is_ok());

        let migrate_to_addr_1 = "new_address_1";
        let migrate_to_code_hash_1 = "code_hash_1";
        let migrate_1_result = migrate(deps.as_mut(), &admin_info, migrate_to_addr_1, migrate_to_code_hash_1);
        assert_eq!(false, migrate_1_result.is_ok());
        assert_eq!(
            migrate_1_result.err().unwrap(),
            StdError::generic_err(format!(
                "This contract has been migrated to {:?}. No further state changes are allowed!",
                Addr::unchecked(migrate_to_addr_0),
            ), )
        );
    }

    #[test]
    fn export_migration_data_three_tokens() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];
        let admin_info = mock_info("creator", &[]);
        let minter_0_info = mock_info("minty_0", &prices.clone());
        let minter_1_info = mock_info("minty_1", &prices.clone());
        let minter_2_info = mock_info("minty_2", &prices.clone());

        let mut deps = mock_dependencies();
        let public_metadata = Some(Metadata { token_uri: Some("public_metadata_uri".to_string()), extension: None });
        let private_metadata = Some(Metadata { token_uri: Some("private_metadata_uri".to_string()), extension: None });
        let instantiate_msg = instantiate_msg(prices.clone(), public_metadata.clone(), private_metadata.clone(), admin_info.clone());
        let _res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        ).unwrap();

        let exec_purchase_msg = ExecuteMsg::Ext(ExecuteMsgExt::PurchaseMint {});
        execute(
            deps.as_mut(),
            mock_env(),
            minter_0_info.clone(),
            exec_purchase_msg.clone(),
        ).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            minter_1_info.clone(),
            exec_purchase_msg.clone(),
        ).unwrap();
        execute(
            deps.as_mut(),
            mock_env(),
            minter_2_info.clone(),
            exec_purchase_msg,
        ).unwrap();

        let migrate_to_addr_0 = "new_address";
        let migrate_to_code_hash_0 = "code_hash";

        let migrate_0_result = migrate(deps.as_mut(), &admin_info, migrate_to_addr_0, migrate_to_code_hash_0);
        assert_eq!(migrate_0_result.is_ok(), true);

        let migrate_data: StdResult<MigrationContractTargetExecuteMsg> = match &migrate_0_result.unwrap().messages[0].msg {
            CosmosMsg::Wasm(msg) => match msg {
                WasmMsg::Execute { contract_addr: _, code_hash: _, msg, funds: _ } => {
                    from_binary(msg)
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        };

        let secret: Binary = match migrate_data {
            Ok(MigrationContractTargetExecuteMsg::SetMigrationSecret { secret }) => secret,
            _ => panic!("unexpected")
        };

        let migration_data = export_migration_data(
            deps.as_ref(),
            vec!["0", "1", "2"].into_iter().map(|id| id.to_string()).collect(),
            secret,
        );

        let first_token = migration_data[0].clone();
        assert_eq!("0", first_token.token_id.clone());
        assert_eq!(public_metadata.clone().unwrap(), first_token.public_metadata.unwrap());
        assert_eq!(private_metadata.clone().unwrap(), first_token.private_metadata.unwrap());
        assert_eq!(minter_0_info.sender, first_token.owner.unwrap());

        let second_token = migration_data[1].clone();
        assert_eq!("1", second_token.token_id.clone());
        assert_eq!(public_metadata.clone().unwrap(), second_token.public_metadata.unwrap());
        assert_eq!(private_metadata.clone().unwrap(), second_token.private_metadata.unwrap());
        assert_eq!(minter_1_info.sender, second_token.owner.unwrap());

        let third_token = migration_data[2].clone();
        assert_eq!("2", third_token.token_id.clone());
        assert_eq!(public_metadata.unwrap(), third_token.public_metadata.unwrap());
        assert_eq!(private_metadata.unwrap(), third_token.private_metadata.unwrap());
        assert_eq!(minter_2_info.sender, third_token.owner.unwrap());
    }
}
