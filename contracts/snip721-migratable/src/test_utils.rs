#[cfg(test)]
pub mod test_utils {
    use cosmwasm_std::MessageInfo;
    use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;

    use crate::msg::InstantiateMsg;

    pub fn instantiate_msg(admin_info: MessageInfo) -> InstantiateMsg {
        InstantiateMsg::New(Snip721InstantiateMsg {
            name: "migratable_snip721".to_string(),
            admin: Some(admin_info.sender.to_string()),
            entropy: "".to_string(),
            royalty_info: None,
            config: None,
            symbol: "".to_string(),
            post_init_callback: None,
            post_init_data: None,
        })
    }
}
