#[cfg(test)]
pub mod test_utils {
    use cosmwasm_std::testing::mock_info;
    use cosmwasm_std::{Event, MessageInfo, Reply, SubMsgResponse, SubMsgResult};

    use crate::msg::InstantiateMsg;

    pub fn admin_msg_info() -> MessageInfo {
        mock_info("admin", &[])
    }

    pub fn child_snip721_address() -> String {
        "child_snip721_addr".to_string()
    }

    pub fn child_snip721_code_hash() -> String {
        "child_snip721_code_hash".to_string()
    }

    pub fn successful_child_snip721_instantiate_reply(child_snip721_address: &str) -> Reply {
        Reply {
            id: 1u64,
            result: SubMsgResult::Ok(SubMsgResponse {
                data: None,
                events: vec![Event::new("instantiate")
                    .add_attribute("contract_address", child_snip721_address)],
            }),
        }
    }

    impl Default for InstantiateMsg {
        fn default() -> Self {
            InstantiateMsg {
                snip721_code_hash: child_snip721_code_hash(),
                snip721_code_id: 10,
                snip721_label: "test_snip721_label".to_string(),
                prices: vec![],
                public_metadata: None,
                private_metadata: None,
                admin: None,
                entropy: "".to_string(),
                royalty_info: None,
            }
        }
    }
}
