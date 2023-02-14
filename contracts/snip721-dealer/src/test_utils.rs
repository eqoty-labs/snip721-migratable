#[cfg(test)]
pub mod test_utils {
    use cosmwasm_std::MessageInfo;
    use cosmwasm_std::testing::mock_info;

    use crate::msg::{CodeInfo, InstantiateSelfAnChildSnip721Msg};

    pub fn admin_msg_info() -> MessageInfo {
        mock_info("admin", &[])
    }

    impl Default for InstantiateSelfAnChildSnip721Msg {
        fn default() -> Self {
            InstantiateSelfAnChildSnip721Msg {
                snip721_code_info: CodeInfo { code_id: 0, code_hash: "".to_string() },
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
