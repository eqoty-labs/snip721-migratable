use cosmwasm_std::{Addr, BankMsg, Binary, BlockInfo, CanonicalAddr, CosmosMsg, Deps, DepsMut, entry_point, Env, from_binary, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, to_binary, WasmMsg};
use cosmwasm_storage::ReadonlyPrefixedStorage;
use secret_toolkit::crypto::Prng;
use secret_toolkit::permit::{Permit, validate};
use secret_toolkit::viewing_key::{ViewingKey, ViewingKeyStore};
use snip721_reference_impl::contract::{gen_snip721_approvals, get_token, mint, mint_list, OwnerInfo, PermissionTypeInfo};
use snip721_reference_impl::expiration::Expiration;
use snip721_reference_impl::mint_run::{SerialNumber, StoredMintRunInfo};
use snip721_reference_impl::msg::{BatchNftDossierElement, ContractStatus, InstantiateConfig, InstantiateMsg as Snip721InstantiateMsg, Mint};
use snip721_reference_impl::royalties::{Royalty, RoyaltyInfo, StoredRoyaltyInfo};
use snip721_reference_impl::state::{Config, CONFIG_KEY, CREATOR_KEY, json_may_load, load, may_load, Permission, PermissionType, PREFIX_ALL_PERMISSIONS, PREFIX_MAP_TO_ID, PREFIX_MINT_RUN, PREFIX_OWNER_PRIV, PREFIX_PRIV_META, PREFIX_PUB_META, PREFIX_REVOKED_PERMITS, PREFIX_ROYALTY_INFO};
use snip721_reference_impl::token::Metadata;

use crate::msg::{ExecuteAnswer, ExecuteMsg, ExecuteMsgExt, InstantiateByMigrationReplyDataMsg, InstantiateMsg, MigrateFrom, MigrateTo, QueryAnswer, QueryMsg, QueryMsgExt};
use crate::msg::QueryAnswer::MigrationBatchNftDossier;
use crate::state::{config, config_read, ContractMode, State};

const MIGRATE_REPLY_ID: u64 = 1u64;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let mut deps = deps;
    if msg.migrate_from.is_none() {
        return init_snip721(&mut deps, &env, info, msg);
    } else {
        let migrate_from = msg.migrate_from.unwrap();
        let migrate_msg = ExecuteMsg::Ext(ExecuteMsgExt::Migrate {
            admin_permit: migrate_from.admin_permit,
            migrate_to: MigrateTo {
                address: env.contract.address.to_string(),
                code_hash: env.contract.code_hash,
                entropy: msg.entropy,
            },
        });
        let migrate_wasm_msg: WasmMsg = WasmMsg::Execute {
            contract_addr: migrate_from.address,
            code_hash: migrate_from.code_hash,
            msg: to_binary(&migrate_msg)?,
            funds: vec![],
        };

        let migrate_submessage = SubMsg::reply_on_success(
            migrate_wasm_msg,
            MIGRATE_REPLY_ID,
        );


        return Ok(Response::new()
            .add_submessages([migrate_submessage])
        );
    }
}

fn init_snip721(
    deps: &mut DepsMut,
    env: &Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let prices = msg.prices.unwrap();
    if prices.len() == 0 {
        return Err(StdError::generic_err(format!(
            "No purchase prices were specified"
        )));
    }
    let state = State {
        prices: prices,
        public_metadata: msg.public_metadata,
        private_metadata: msg.private_metadata,
        migration_addr: None,
        migration_code_hash: None,
        migration_secret: None,
        migrate_in_mint_cnt: None,
        migrate_in_next_mint_index: None,
        mode: ContractMode::Running,
    };

    config(deps.storage).save(&state)?;
    let instantiate_msg = Snip721InstantiateMsg {
        name: "PurchasableSnip721".to_string(),
        symbol: "PUR721".to_string(),
        admin: msg.admin.clone(),
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
    snip721_reference_impl::contract::instantiate(deps, env, info.clone(), instantiate_msg)
        .unwrap();

    deps.api
        .debug(format!("PurchasableSnip721 was initialized by {}", info.sender).as_str());

    return Ok(Response::default());
}


#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let mut deps = deps;
    let mut state = config_read(deps.storage).load()?;
    if let ContractMode::MigratedOut = state.mode {
        return Err(StdError::generic_err(format!(
            "This contract has been migrated to {:?}. No further state changes are allowed!",
            state.migration_addr.unwrap()
        )));
    } else if ContractMode::MigrateDataIn == state.mode {
        return perform_token_migration(deps, &env, info, config, msg);
    }

    return match msg {
        ExecuteMsg::Base(base_msg) => {
            snip721_reference_impl::contract::execute(deps, env, info, base_msg)
        }
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::PurchaseMint { .. } => {
                purchase_and_mint(&mut deps, env, info, &mut config, &mut state)
            }
            ExecuteMsgExt::Migrate { admin_permit, migrate_to } =>
                migrate(deps, env, info, &mut config, &mut state, admin_permit, migrate_to),
            ExecuteMsgExt::MigrateTokensIn { .. } => {
                Err(StdError::generic_err(
                    "MigrateTokensIn msg is allowed when in ContractMode:MigrateDataIn",
                ))
            }
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


fn perform_token_migration(deps: DepsMut, env: &Env, info: MessageInfo, snip721_config: Config, msg: ExecuteMsg) -> StdResult<Response> {
    let mut state = config_read(deps.storage).load()?;
    let admin_addr = deps.api.addr_humanize(&snip721_config.admin).unwrap();
    if admin_addr != info.sender {
        return Err(StdError::generic_err(format!(
            "This contract's admin must complete migrating contract data from {:?}",
            state.migration_addr.unwrap()
        )));
    }

    let (pages, page_size) = match msg {
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::MigrateTokensIn { pages, page_size } => {
                (pages.unwrap_or(u32::MAX), page_size)
            }
            _ => {
                return Err(StdError::generic_err(
                    "Only MigrateTokensIn msg is allowed when in ContractMode:MigrateDataIn",
                ));
            }
        },
        _ => {
            return Err(StdError::generic_err(format!(
                "This contract's admin must complete migrating contract data from {:?}",
                state.migration_addr.unwrap()
            )));
        }
    };
    let mut deps = deps;
    let mut start_at_idx = state.migrate_in_next_mint_index.unwrap();
    let mint_count = state.migrate_in_mint_cnt.unwrap();

    let mut pages_queried = 0;
    while pages_queried < pages && start_at_idx < mint_count {
        // deps.api.debug(&*format!("start_at_idx: {} mint_count {}", start_at_idx, mint_count));
        let query_answer: QueryAnswer = deps.querier.query_wasm_smart(
            state.migration_code_hash.clone().unwrap(),
            state.migration_addr.clone().unwrap(),
            &QueryMsgExt::ExportMigrationData {
                start_index: Some(start_at_idx),
                max_count: page_size,
                secret: state.migration_secret.clone().unwrap(),
            },
        ).unwrap();
        pages_queried += 1;
        start_at_idx = match query_answer {
            MigrationBatchNftDossier { last_mint_index, nft_dossiers } => {
                deps.api.debug(format!("{:?}", nft_dossiers).as_str());
                save_migration_dossier_list(&mut deps, env, &admin_addr, nft_dossiers).unwrap();
                last_mint_index + 1
            }
            _ => panic!("unexpected ExportMigrationData query answer"),
        };
    }

    let res = if start_at_idx < mint_count {
        state.migrate_in_next_mint_index = Some(start_at_idx);

        Ok(Response::new()
            .set_data(to_binary(&ExecuteAnswer::MigrateTokensIn {
                complete: false,
                next_mint_index: Some(start_at_idx),
                total: Some(mint_count),
            })?))
    } else {
        // migration complete
        state.migrate_in_next_mint_index = None;
        state.migrate_in_mint_cnt = None;
        state.migration_secret = None;
        state.migration_addr = None;
        state.migration_code_hash = None;
        state.mode = ContractMode::Running;

        Ok(Response::new()
            .set_data(to_binary(&ExecuteAnswer::MigrateTokensIn {
                complete: true,
                next_mint_index: None,
                total: None,
            })?))
    };
    config(deps.storage).save(&state)?;
    return res;
}

fn save_migration_dossier_list(
    deps: &mut DepsMut,
    env: &Env,
    admin_addr: &Addr,
    nft_dossiers: Vec<BatchNftDossierElement>,
) -> StdResult<Vec<String>> {
    let mints = nft_dossiers.iter().map(|nft| {
        let royalty_info = if let Some(some_royalty_info) = nft.royalty_info.clone() {
            Some(
                RoyaltyInfo {
                    decimal_places_in_rates: some_royalty_info.decimal_places_in_rates,
                    royalties: some_royalty_info.royalties.iter().map(|r|
                        Royalty { recipient: r.recipient.clone().unwrap().to_string(), rate: r.rate }
                    ).collect(),
                }
            )
        } else {
            None
        };

        let mint_run_info = nft.mint_run_info.clone().unwrap();
        let serial_number = if let Some(some_serial_number) = mint_run_info.serial_number {
            Some(
                SerialNumber {
                    mint_run: mint_run_info.mint_run,
                    serial_number: some_serial_number,
                    quantity_minted_this_run: mint_run_info.quantity_minted_this_run,
                }
            )
        } else {
            None
        };
        Mint {
            token_id: Some(nft.token_id.clone()),
            owner: Some(nft.owner.clone().unwrap().to_string()),
            public_metadata: nft.public_metadata.clone(),
            private_metadata: nft.private_metadata.clone(),
            serial_number,
            royalty_info,
            transferable: Some(nft.transferable.clone()),
            memo: None,
        }
    }).collect();
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let sender_raw = &deps.api.addr_canonicalize(admin_addr.as_str())?;
    mint_list(deps, env, &mut config, sender_raw, mints)
}

pub fn migrate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    snip721config: &mut Config,
    state: &mut State,
    admin_permit: Permit,
    migrate_to: MigrateTo,
) -> StdResult<Response> {
    let admin_addr = &deps.api.addr_humanize(&snip721config.admin).unwrap();
    let permit_creator = &deps.api.addr_validate(
        &validate(
            deps.as_ref(),
            PREFIX_REVOKED_PERMITS,
            &admin_permit,
            env.contract.address.to_string(),
            Some("secret"),
        )?
    ).unwrap();

    if permit_creator != admin_addr {
        return Err(StdError::generic_err(
            "Only the admins permit is allowed to initiate migration!",
        ));
    }
    let migrate_to_address = deps.api.addr_validate(migrate_to.address.as_str()).unwrap();
    if info.sender != migrate_to_address {
        return Err(StdError::generic_err(
            "Only the contract being migrated to can set the contract to migrate!",
        ));
    }
    if state.migration_addr.is_some() {
        return Err(StdError::generic_err(
            "The contract has already been migrated!",
        ));
    }
    let entropy = migrate_to.entropy.as_str();

    // Generate the secret in some way
    // 16 here represents the lengths in bytes of the block height and time.
    let entropy_len = 16 + info.sender.to_string().len() + entropy.len();
    let mut rng_entropy = Vec::with_capacity(entropy_len);
    rng_entropy.extend_from_slice(&env.block.height.to_be_bytes());
    rng_entropy.extend_from_slice(&env.block.time.seconds().to_be_bytes());
    rng_entropy.extend_from_slice(info.sender.as_bytes());
    rng_entropy.extend_from_slice(entropy.as_ref());
    const SEED_KEY: &[u8] = b"::seed";
    let mut seed_key = Vec::with_capacity(ViewingKey::STORAGE_KEY.len() + SEED_KEY.len());
    seed_key.extend_from_slice(ViewingKey::STORAGE_KEY);
    seed_key.extend_from_slice(SEED_KEY);
    let seed = &deps.storage.get(&seed_key).unwrap_or_default();

    let mut rng = Prng::new(seed, &rng_entropy);

    let secret = Binary::from(rng.rand_bytes());

    state.migration_addr = Some(migrate_to_address);
    state.migration_code_hash = Some(migrate_to.code_hash);
    state.mode = ContractMode::MigratedOut;
    state.migration_secret = Some(secret.clone());
    config(deps.storage).save(&state)?;

    Ok(Response::default()
        .set_data(to_binary(&InstantiateByMigrationReplyDataMsg {
            migrated_instantiate_msg: InstantiateMsg {
                migrate_from: None,
                prices: Some(state.prices.clone()),
                public_metadata: state.public_metadata.clone(),
                private_metadata: state.private_metadata.clone(),
                admin: Some(admin_addr.to_string()),
                entropy: entropy.to_string(),
                royalty_info: None,
            },
            migrate_from: MigrateFrom {
                address: env.contract.address.to_string(),
                code_hash: env.contract.code_hash,
                admin_permit,
            },
            mint_count: snip721config.mint_cnt,
            secret,
        }).unwrap())
    )
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        MIGRATE_REPLY_ID => instantiate_with_migrated_config(deps, &env, msg),
        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

fn instantiate_with_migrated_config(deps: DepsMut, env: &Env, msg: Reply) -> StdResult<Response> {
    let mut deps = deps;
    deps.api.debug(&*format!("msg.result: {:?}!", msg.result.clone().unwrap()));

    let reply_data: InstantiateByMigrationReplyDataMsg = from_binary(&msg.result.unwrap().data.unwrap()).unwrap();
    // admin of the contract being migrated should always be the sender here
    let admin_info = MessageInfo {
        sender: deps.api.addr_validate(reply_data.migrated_instantiate_msg.admin.clone().unwrap().as_str()).unwrap(),
        funds: vec![],
    };
    // actually instantiate the contract using the migrated data
    init_snip721(&mut deps, env, admin_info.clone(), reply_data.migrated_instantiate_msg).unwrap();

    let mut state = config_read(deps.storage).load()?;
    state.migration_addr = Some(deps.api.addr_validate(reply_data.migrate_from.address.as_str()).unwrap());
    state.migration_code_hash = Some(reply_data.migrate_from.code_hash);
    state.mode = ContractMode::MigrateDataIn;
    state.migration_secret = Some(reply_data.secret);
    state.migrate_in_next_mint_index = Some(0);
    state.migrate_in_mint_cnt = Some(reply_data.mint_count);
    config(deps.storage).save(&state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    return match msg {
        QueryMsg::Base(base_msg) => snip721_reference_impl::contract::query(deps, env, base_msg),
        QueryMsg::Ext(ext_msg) => match ext_msg {
            QueryMsgExt::GetPrices {} => query_prices(deps),
            QueryMsgExt::ExportMigrationData { start_index, max_count, secret } =>
                migration_dossier_list(deps, &env.block, start_index, max_count, secret)
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

/// Returns StdResult<Binary(MigrationBatchNftDossier)> of all the token information for multiple tokens.
/// This can only be used by the contract being migrated to at migration_addr
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
/// * `block` - a reference to the BlockInfo
/// * `start_index` - optionally only display token starting at this index
/// * `max_count` - optional max number of tokens to display
/// * `secret` - the migration secret
pub fn migration_dossier_list(
    deps: Deps,
    block: &BlockInfo,
    start_index: Option<u32>,
    max_count: Option<u32>,
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

    let max_count = max_count.unwrap_or(300);
    let mut count = 0u32;
    let mut idx: u32 = start_index.unwrap_or(0);
    let map2id = ReadonlyPrefixedStorage::new(deps.storage, PREFIX_MAP_TO_ID);
    while count < max_count && idx < config.mint_cnt {
        if let Some(id) = may_load::<String>(&map2id, &idx.to_le_bytes())? {
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
            count += 1;
        }
        // idx can't overflow if it was less than a u32
        idx += 1;
    }
    to_binary(&MigrationBatchNftDossier { last_mint_index: idx, nft_dossiers: dossiers })
}
