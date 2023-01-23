use cosmwasm_std::{
    BankMsg, Binary, BlockInfo, CanonicalAddr, CosmosMsg, Deps, DepsMut, entry_point, Env,
    MessageInfo, Response, StdError, StdResult, to_binary, WasmMsg,
};
use cosmwasm_storage::ReadonlyPrefixedStorage;
use snip721_reference_impl::contract::{
    gen_snip721_approvals, get_token, mint, OwnerInfo, PermissionTypeInfo,
};
use snip721_reference_impl::expiration::Expiration;
use snip721_reference_impl::mint_run::StoredMintRunInfo;
use snip721_reference_impl::msg::{
    BatchNftDossierElement, ContractStatus, InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg,
};
use snip721_reference_impl::msg::QueryAnswer::BatchNftDossier;
use snip721_reference_impl::royalties::StoredRoyaltyInfo;
use snip721_reference_impl::state::{
    Config, CONFIG_KEY, CREATOR_KEY, json_may_load, load, may_load, Permission, PermissionType,
    PREFIX_ALL_PERMISSIONS, PREFIX_MINT_RUN, PREFIX_OWNER_PRIV, PREFIX_PRIV_META, PREFIX_PUB_META,
    PREFIX_ROYALTY_INFO,
};
use snip721_reference_impl::token::Metadata;

use crate::msg::{
    ExecuteMsg, ExecuteMsgExt, InstantiateMsg, MigrationContractTargetExecuteMsg, QueryAnswer,
    QueryMsg, QueryMsgExt,
};
use crate::state::{config, config_read, ContractMode, State};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    if msg.prices.len() == 0 {
        return Err(StdError::generic_err(format!(
            "No purchase prices were specified"
        )));
    }
    let state = State {
        prices: msg.prices,
        public_metadata: msg.public_metadata,
        private_metadata: msg.private_metadata,
        migration_addr: None,
        migration_secret: None,
        mode: ContractMode::Running,
    };

    config(deps.storage).save(&state)?;
    let instantiate_msg = Snip721InstantiateMsg {
        name: "PurchasableSnip721".to_string(),
        symbol: "PUR721".to_string(),
        admin: Some(msg.admin.clone()),
        entropy: msg.entropy,
        royalty_info: msg.royalty_info,
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
    snip721_reference_impl::contract::instantiate(&mut deps, env, info.clone(), instantiate_msg)
        .unwrap();

    deps.api
        .debug(format!("PurchasableSnip721 was initialized by {}", info.sender).as_str());
    Ok(Response::default())
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let mut deps = deps;
    let mut state = config_read(deps.storage).load()?;
    if let ContractMode::Migrated = state.mode {
        return Err(StdError::generic_err(format!(
            "This contract has been migrated to {:?}. No further state changes are allowed!",
            state.migration_addr.unwrap()
        )));
    }

    return match msg {
        ExecuteMsg::Base(base_msg) => {
            snip721_reference_impl::contract::execute(deps, env, info, base_msg)
        }
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::PurchaseMint { .. } => {
                purchase_and_mint(&mut deps, env, info, &mut config, &mut state)
            }
            ExecuteMsgExt::Migrate { address, code_hash } =>
                migrate(deps, info, &mut config, &mut state, address, code_hash),
        },
    };
}

fn purchase_and_mint(
    deps: &mut DepsMut,
    env: Env,
    info: MessageInfo,
    config: &mut Config,
    state: &mut State,
) -> StdResult<Response> {
    if info.funds.len() != 1 {
        return Err(StdError::generic_err(format!(
            "Purchase requires one coin denom to be sent with transaction, {} were sent.",
            info.funds.len()
        )));
    }
    let msg_fund = &info.funds[0];
    let selected_coin_price = state.prices.iter().find(|c| c.denom == msg_fund.denom);
    if let Some(selected_coin_price) = selected_coin_price {
        if msg_fund.amount != selected_coin_price.amount {
            return Err(StdError::generic_err(format!(
                "Purchase price in {} is {}, but {} was sent",
                selected_coin_price.denom, selected_coin_price.amount, msg_fund
            )));
        }
    } else {
        return Err(StdError::generic_err(format!(
            "Purchasing in denom:{} is not allowed",
            msg_fund.denom
        )));
    }
    let sender = info.clone().sender;
    let pay_to_addr = deps.api.addr_humanize(&config.admin).unwrap();
    let send_funds_messages = vec![CosmosMsg::Bank(BankMsg::Send {
        to_address: pay_to_addr.to_string(),
        amount: info.funds.clone(),
    })];
    let admin_addr = deps.api.addr_humanize(&config.admin).unwrap();

    let mint_result = mint(
        deps,
        &env,
        &admin_addr,
        config,
        ContractStatus::Normal.to_u8(),
        None,
        Some(sender.to_string()),
        state.public_metadata.clone(),
        state.private_metadata.clone(),
        None,
        None,
        None,
        None,
    );
    if let Err(mint_err) = mint_result {
        return Err(mint_err);
    };
    let mint_res = mint_result.unwrap().clone();
    Ok(Response::new()
        .add_messages(send_funds_messages)
        .add_attributes(mint_res.attributes)
        .set_data(mint_res.data.unwrap()))
}

pub fn migrate(
    deps: DepsMut,
    info: MessageInfo,
    snip721config: &mut Config,
    state: &mut State,
    address: String,
    code_hash: String,
) -> StdResult<Response> {
    let admin_addr = deps.api.addr_humanize(&snip721config.admin).unwrap();
    if info.sender != admin_addr {
        return Err(StdError::generic_err(
            "Only the admin can set the contract to migrate!",
        ));
    }
    if state.migration_addr.is_some() {
        return Err(StdError::generic_err(
            "The contract has already been migrated!",
        ));
    }
    let address = deps.api.addr_validate(&address).unwrap();

    // Generate the secret in some way
    let secret = Binary::from(b"asdfgh");

    state.migration_addr = Some(address.clone());
    state.mode = ContractMode::Migrated;
    state.migration_secret = Some(secret.clone());
    config(deps.storage).save(&state)?;

    let messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        msg: to_binary(&MigrationContractTargetExecuteMsg::SetMigrationSecret { secret })?,
        contract_addr: address.to_string(),
        code_hash,
        funds: vec![],
    })];
    Ok(Response::default().add_messages(messages))
}


#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    return match msg {
        QueryMsg::Base(base_msg) => snip721_reference_impl::contract::query(deps, env, base_msg),
        QueryMsg::Ext(ext_msg) => match ext_msg {
            QueryMsgExt::GetPrices {} => query_prices(deps),
            QueryMsgExt::ExportMigrationData { token_ids, secret } =>
                migration_dossier_list(deps, &env.block, token_ids, secret)
        },
    };
}

/// Returns StdResult<Binary> displaying prices to mint in all acceptable currency denoms
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
pub fn query_prices(deps: Deps) -> StdResult<Binary> {
    let state = config_read(deps.storage).load()?;

    to_binary(&QueryAnswer::GetPrices {
        prices: state.prices,
    })
}

/// Returns StdResult<Binary(Vec<BatchNftDossierElement>)> of all the token information for multiple tokens.
/// This can only be used by the contract being migrated to at migration_addr
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
/// * `block` - a reference to the BlockInfo
/// * `token_ids` - list of token ids to retrieve the info of
/// * `secret` - the migration secret
pub fn migration_dossier_list(
    deps: Deps,
    block: &BlockInfo,
    token_ids: Vec<String>,
    secret: Binary,
) -> StdResult<Binary> {
    let state = config_read(deps.storage).load()?;
    let migration_secret = state
        .migration_secret
        .ok_or_else(|| StdError::generic_err("This contract has not been migrated yet"))?;
    if migration_secret != secret {
        return Err(StdError::generic_err(
            "This contract has not been migrated yet",
        ));
    }

    let incl_exp = true;
    let config: Config = load(deps.storage, CONFIG_KEY)?;
    let contract_creator = deps
        .api
        .addr_humanize(&load::<CanonicalAddr>(deps.storage, CREATOR_KEY)?)?;

    let perm_type_info = PermissionTypeInfo {
        view_owner_idx: PermissionType::ViewOwner.to_usize(),
        view_meta_idx: PermissionType::ViewMetadata.to_usize(),
        transfer_idx: PermissionType::Transfer.to_usize(),
        num_types: PermissionType::Transfer.num_types(),
    };
    // used to shortcut permission checks if the viewer is already a known operator for a list of owners
    let mut owner_cache: Vec<OwnerInfo> = Vec::new();
    let mut dossiers: Vec<BatchNftDossierElement> = Vec::new();
    // set up all the immutable storage references
    let own_priv_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_OWNER_PRIV);
    let pub_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_PUB_META);
    let priv_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_PRIV_META);
    let roy_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_ROYALTY_INFO);
    let run_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_MINT_RUN);
    let all_store = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_ALL_PERMISSIONS);

    for id in token_ids.into_iter() {
        let (mut token, idx) = get_token(deps.storage, &id, None)?;
        let owner_slice = token.owner.as_slice();
        // get the owner info either from the cache or storage
        let owner_inf = if let Some(inf) = owner_cache.iter().find(|o| o.owner == token.owner) {
            inf
        } else {
            let owner_is_public: bool =
                may_load(&own_priv_store, owner_slice)?.unwrap_or(config.owner_is_public);
            let mut all_perm: Vec<Permission> =
                json_may_load(&all_store, owner_slice)?.unwrap_or_default();
            let (inventory_approvals, view_owner_exp, view_meta_exp) =
                gen_snip721_approvals(deps.api, block, &mut all_perm, incl_exp, &perm_type_info)?;
            owner_cache.push(OwnerInfo {
                owner: token.owner.clone(),
                owner_is_public,
                inventory_approvals,
                view_owner_exp,
                view_meta_exp,
            });
            owner_cache.last().ok_or_else(|| {
                StdError::generic_err("This can't happen since we just pushed an OwnerInfo!")
            })?
        };
        let global_pass = owner_inf.owner_is_public;
        // get the owner
        let owner = Some(deps.api.addr_humanize(&token.owner)?);
        // get the public metadata
        let token_key = idx.to_le_bytes();
        let public_metadata: Option<Metadata> = may_load(&pub_store, &token_key)?;
        // get the private metadata if it is not sealed and if the viewer is permitted
        let display_private_metadata_error = None;
        let private_metadata: Option<Metadata> = may_load(&priv_store, &token_key)?;
        // get the royalty information if present
        let may_roy_inf: Option<StoredRoyaltyInfo> = may_load(&roy_store, &token_key)?;
        let royalty_info = may_roy_inf
            .map(|r| {
                r.to_human(deps.api, false)
            })
            .transpose()?;
        // get the mint run information
        let mint_run: StoredMintRunInfo = load(&run_store, &token_key)?;
        // get the token approvals
        let (token_approv, token_owner_exp, token_meta_exp) = gen_snip721_approvals(
            deps.api,
            block,
            &mut token.permissions,
            incl_exp,
            &perm_type_info,
        )?;
        // determine if ownership is public
        let (public_ownership_expiration, owner_is_public) = if global_pass {
            (Some(Expiration::Never), true)
        } else if token_owner_exp.is_some() {
            (token_owner_exp, true)
        } else {
            (
                owner_inf.view_owner_exp.as_ref().cloned(),
                owner_inf.view_owner_exp.is_some(),
            )
        };
        // determine if private metadata is public
        let (private_metadata_is_public_expiration, private_metadata_is_public) =
            if token_meta_exp.is_some() {
                (token_meta_exp, true)
            } else {
                (
                    owner_inf.view_meta_exp.as_ref().cloned(),
                    owner_inf.view_meta_exp.is_some(),
                )
            };
        // display the approvals
        let (token_approvals, inventory_approvals) = (
            Some(token_approv),
            Some(owner_inf.inventory_approvals.clone()),
        );
        dossiers.push(BatchNftDossierElement {
            token_id: id,
            owner,
            public_metadata,
            private_metadata,
            royalty_info,
            mint_run_info: Some(mint_run.to_human(deps.api, contract_creator.clone())?),
            transferable: token.transferable,
            unwrapped: token.unwrapped,
            display_private_metadata_error,
            owner_is_public,
            public_ownership_expiration,
            private_metadata_is_public,
            private_metadata_is_public_expiration,
            token_approvals,
            inventory_approvals,
        });
    }
    to_binary(&BatchNftDossier { nft_dossiers: dossiers })
}
