#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, BankMsg, Coin, CosmosMsg, Deps, DepsMut, from_binary, MessageInfo, Response, StdError, StdResult, Uint128};
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

        let instantiate_msg = instantiate_msg(prices.clone(), admin_info.clone());
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
        assert_eq!(migrate_0_result.is_ok(), true);

        let migrate_to_addr_1 = "new_address_1";
        let migrate_to_code_hash_1 = "code_hash_1";
        let migrate_1_result = migrate(deps.as_mut(), &admin_info, migrate_to_addr_1, migrate_to_code_hash_1);
        assert_eq!(migrate_1_result.is_ok(), false);
        assert_eq!(
            migrate_1_result.err().unwrap(),
            StdError::generic_err(format!(
                "This contract has been migrated to {:?}. No further state changes are allowed!",
                Addr::unchecked(migrate_to_addr_0),
            ), )
        );
    }
}
