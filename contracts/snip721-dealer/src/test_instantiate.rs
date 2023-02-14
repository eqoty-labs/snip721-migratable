#[cfg(test)]
mod tests {
    use cosmwasm_std::{Api, CanonicalAddr, Coin, CosmosMsg, from_binary, ReplyOn, StdError, Uint128, WasmMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use snip721_reference_impl::state::load;
    use snip721_reference_impl::msg::{InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg};
    use snip721_reference_impl::token::Metadata;

    use crate::contract::instantiate;
    use crate::msg::{InstantiateMsg, InstantiateSelfAnChildSnip721Msg};
    use crate::state::{ADMIN_KEY, PURCHASABLE_METADATA_KEY, PurchasableMetadata, PURCHASE_PRICES_KEY};
    use crate::test_utils::test_utils::admin_msg_info;

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

        let admin_info = admin_msg_info();

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateMsg::New(InstantiateSelfAnChildSnip721Msg {
            admin: Some(admin_info.sender.to_string()),
            prices: prices.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAnChildSnip721Msg::default()
        });

        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        );
        assert!(res.is_ok(),);

        let saved_prices: Vec<Coin> = load(deps.as_ref().storage, PURCHASE_PRICES_KEY).unwrap();
        assert_eq!(prices, saved_prices);
        let saved_purchasable_metadata: PurchasableMetadata = load(deps.as_ref().storage, PURCHASABLE_METADATA_KEY).unwrap();
        assert_eq!(purchasable_metadata, saved_purchasable_metadata);
        let saved_admin: CanonicalAddr = load(deps.as_ref().storage, ADMIN_KEY).unwrap();
        assert_eq!(deps.api.addr_canonicalize(admin_info.sender.as_str()).unwrap(), saved_admin);
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

        let instantiate_msg = InstantiateMsg::New(InstantiateSelfAnChildSnip721Msg {
            admin: None,
            prices: prices.clone(),
            private_metadata: purchasable_metadata.private_metadata.clone(),
            public_metadata: purchasable_metadata.public_metadata.clone(),
            ..InstantiateSelfAnChildSnip721Msg::default()
        });

        let admin_info = admin_msg_info();
        instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        ).unwrap();

        let saved_admin: CanonicalAddr = load(deps.as_ref().storage, ADMIN_KEY).unwrap();
        assert_eq!(deps.api.addr_canonicalize(admin_info.sender.as_str()).unwrap(), saved_admin);
    }

    #[test]
    fn instantiate_with_no_prices_fails() {
        let prices = vec![];

        let admin_info = mock_info("creator", &[]);
        let _minter_info = mock_info("minty", &prices.clone());

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateMsg::New(InstantiateSelfAnChildSnip721Msg {
            admin: Some(admin_info.sender.to_string()),
            prices,
            ..InstantiateSelfAnChildSnip721Msg::default()
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
    fn instantiate_new_adds_submessage_to_instantiate_child_snip721() {
        let prices = vec![Coin {
            amount: Uint128::new(100),
            denom: "`uscrt`".to_string(),
        }];

        let mut deps = mock_dependencies();

        let instantiate_msg = InstantiateSelfAnChildSnip721Msg {
            prices: prices.clone(),
            ..InstantiateSelfAnChildSnip721Msg::default()
        };

        let admin_info = admin_msg_info();
        let res = instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            InstantiateMsg::New(instantiate_msg.clone()),
        ).unwrap();

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
                    code_id, code_hash, msg, funds, label
                } => {
                    assert_eq!(&instantiate_msg.snip721_code_info.code_id, code_id);
                    assert_eq!(&instantiate_msg.snip721_code_info.code_hash, code_hash);
                    assert_eq!(&Vec::<Coin>::new(), funds);
                    assert_eq!(&instantiate_msg.snip721_label, label);
                    let snip721_instantiate_msg: Snip721InstantiateMsg = from_binary(msg).unwrap();
                    let expected_snip721_instantiate_msg = Snip721InstantiateMsg {
                        name: "PurchasableSnip721".to_string(),
                        symbol: "PUR721".to_string(),
                        admin: Some(admin_info.sender.to_string()),
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
                    };
                    assert_eq!(expected_snip721_instantiate_msg, snip721_instantiate_msg);
                }
                _ => panic!("unexpected"),
            },
            _ => panic!("unexpected"),
        }
    }
}
