use cosmwasm_std::{
    from_binary, to_binary, Addr, Api, Binary, BlockInfo, CanonicalAddr, Deps, DepsMut, Env,
    MessageInfo, Reply, Response, StdError, StdResult, SubMsg, WasmMsg,
};
use cosmwasm_storage::ReadonlyPrefixedStorage;
use cw_migratable_contract_std::execute::check_contract_mode;
use cw_migratable_contract_std::msg::MigrationListenerExecuteMsg;
use cw_migratable_contract_std::msg_types::{MigrateFrom, MigrateTo};
use cw_migratable_contract_std::state::{
    canonicalize, CanonicalContractInfo, ContractMode, MigratedFromState, MigratedToState,
    CONTRACT_MODE, MIGRATED_FROM, MIGRATED_TO, MIGRATION_COMPLETE_EVENT_SUBSCRIBERS,
    REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS,
};
use secret_toolkit::crypto::ContractPrng;
use secret_toolkit::permit::{validate, Permit};
use secret_toolkit::viewing_key::{ViewingKey, ViewingKeyStore};
use snip721_reference_impl::contract::{
    gen_snip721_approvals, get_token, mint_list, OwnerInfo, PermissionTypeInfo,
};
use snip721_reference_impl::expiration::Expiration;
use snip721_reference_impl::mint_run::{SerialNumber, StoredMintRunInfo};
use snip721_reference_impl::msg::InstantiateMsg as Snip721InstantiateMsg;
use snip721_reference_impl::msg::{BatchNftDossierElement, InstantiateConfig, Mint};
use snip721_reference_impl::royalties::{Royalty, RoyaltyInfo, StoredRoyaltyInfo};
use snip721_reference_impl::state::{
    json_may_load, load, may_load, save, Config, Permission, PermissionType, CONFIG_KEY,
    CREATOR_KEY, DEFAULT_ROYALTY_KEY, MINTERS_KEY, PREFIX_ALL_PERMISSIONS, PREFIX_MAP_TO_ID,
    PREFIX_MINT_RUN, PREFIX_OWNER_PRIV, PREFIX_PRIV_META, PREFIX_PUB_META, PREFIX_REVOKED_PERMITS,
    PREFIX_ROYALTY_INFO,
};
use snip721_reference_impl::token::Metadata;

use crate::contract::init_snip721;
use crate::msg::QueryAnswer::MigrationBatchNftDossier;
use crate::msg::{ExecuteAnswer, InstantiateByMigrationReplyDataMsg, QueryAnswer, QueryMsgExt};
use crate::state::{MigrateInTokensProgress, MIGRATE_IN_TOKENS_PROGRESS};

pub(crate) fn instantiate_with_migrated_config(
    deps: DepsMut,
    env: &Env,
    msg: Reply,
) -> StdResult<Response> {
    let mut deps = deps;
    let reply_data: InstantiateByMigrationReplyDataMsg =
        from_binary(&msg.result.unwrap().data.unwrap())?;
    // admin of the contract being migrated should always be the sender here
    let admin_info = MessageInfo {
        sender: deps.api.addr_validate(
            reply_data
                .migrated_instantiate_msg
                .admin
                .clone()
                .unwrap()
                .as_str(),
        )?,
        funds: vec![],
    };

    // actually instantiate the snip721 base contract using the migrated data
    let snip721_response = init_snip721(
        &mut deps,
        env,
        admin_info,
        reply_data.migrated_instantiate_msg,
    )
    .unwrap();

    let migrated_from = MigratedFromState {
        contract: CanonicalContractInfo {
            address: deps
                .api
                .addr_canonicalize(reply_data.migrate_from.address.as_str())?,
            code_hash: reply_data.migrate_from.code_hash,
        },
        migration_secret: reply_data.secret,
    };
    MIGRATED_FROM.save(deps.storage, &migrated_from)?;
    let migrate_in_tokens_progress = MigrateInTokensProgress {
        migrate_in_mint_cnt: reply_data.mint_count,
        migrate_in_next_mint_index: 0,
    };
    MIGRATE_IN_TOKENS_PROGRESS.save(deps.storage, &migrate_in_tokens_progress)?;
    save(deps.storage, MINTERS_KEY, &reply_data.minters)?;

    CONTRACT_MODE.save(deps.storage, &ContractMode::MigrateDataIn)?;
    REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS.save(
        deps.storage,
        &reply_data.remaining_migration_complete_event_sub_slots,
    )?;
    if let Some(migration_complete_event_subscribers) =
        reply_data.migration_complete_event_subscribers
    {
        MIGRATION_COMPLETE_EVENT_SUBSCRIBERS.save(
            deps.storage,
            &migration_complete_event_subscribers
                .iter()
                .map(|c| canonicalize(deps.api, c).unwrap())
                .collect(),
        )?;
    }

    // clear the data (that contains the secret) which would be set when init_snip721 is called
    // from reply as part of the migration process
    // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
    Ok(snip721_response.set_data(b""))
}

pub(crate) fn perform_token_migration(
    deps: DepsMut,
    env: &Env,
    info: MessageInfo,
    contract_mode: ContractMode,
    snip721_config: Config,
    page_size: Option<u32>,
) -> StdResult<Response> {
    check_contract_mode(vec![ContractMode::MigrateDataIn], &contract_mode, None)?;
    let migrated_from_state = MIGRATED_FROM.load(deps.storage)?;
    let admin_addr = deps.api.addr_humanize(&snip721_config.admin).unwrap();
    if admin_addr != info.sender {
        return Err(StdError::generic_err(format!(
            "This contract's admin must complete migrating contract data from {:?}",
            migrated_from_state.contract.humanize(deps.api)?
        )));
    }
    let migrated_from = migrated_from_state.contract.into_humanized(deps.api)?;
    let mut deps = deps;
    let mut migrate_in_tokens_progress = MIGRATE_IN_TOKENS_PROGRESS.load(deps.storage)?;
    let mut start_at_idx = migrate_in_tokens_progress.migrate_in_next_mint_index;
    let mint_count = migrate_in_tokens_progress.migrate_in_mint_cnt;

    if start_at_idx < mint_count {
        let query_answer: QueryAnswer = deps
            .querier
            .query_wasm_smart(
                migrated_from.code_hash.clone(),
                migrated_from.address.clone(),
                &QueryMsgExt::ExportMigrationData {
                    start_index: Some(start_at_idx),
                    max_count: page_size,
                    secret: migrated_from_state.migration_secret,
                },
            )
            .unwrap();
        start_at_idx = match query_answer {
            MigrationBatchNftDossier {
                last_mint_index,
                nft_dossiers,
            } => {
                save_migration_dossier_list(
                    &mut deps,
                    env,
                    &migrated_from.address,
                    &admin_addr,
                    nft_dossiers,
                )
                .unwrap();
                last_mint_index + 1
            }
        };
    }
    migrate_in_tokens_progress.migrate_in_next_mint_index = start_at_idx;
    MIGRATE_IN_TOKENS_PROGRESS.save(deps.storage, &migrate_in_tokens_progress)?;

    if start_at_idx < mint_count {
        Ok(
            Response::new().set_data(to_binary(&ExecuteAnswer::MigrateTokensIn {
                complete: false,
                next_mint_index: Some(start_at_idx),
                total: Some(mint_count),
            })?),
        )
    } else {
        // migration complete
        CONTRACT_MODE.save(deps.storage, &ContractMode::Running)?;
        // notify the contract being migrated from so it can change its mode from MigrateOutStarted to MigratedOut
        let msg = to_binary(
            &MigrationListenerExecuteMsg::MigrationCompleteNotification {
                to: env.contract.clone(),
                data: None,
            },
        )?;
        let sub_msgs: Vec<SubMsg> = vec![SubMsg::new(WasmMsg::Execute {
            msg,
            contract_addr: migrated_from.address.to_string(),
            code_hash: migrated_from.code_hash.clone(),
            funds: vec![],
        })];
        Ok(Response::new()
            .add_submessages(sub_msgs)
            .set_data(to_binary(&ExecuteAnswer::MigrateTokensIn {
                complete: true,
                next_mint_index: None,
                total: None,
            })?))
    }
}

fn stored_to_msg_royalty_info(stored: StoredRoyaltyInfo, api: &dyn Api) -> RoyaltyInfo {
    RoyaltyInfo {
        decimal_places_in_rates: stored.decimal_places_in_rates,
        royalties: stored
            .royalties
            .iter()
            .map(|r| {
                Ok(Royalty {
                    recipient: api.addr_humanize(&r.recipient)?.to_string(),
                    rate: r.rate,
                })
            })
            .collect::<StdResult<Vec<Royalty>>>()
            .unwrap(),
    }
}

fn save_migration_dossier_list(
    deps: &mut DepsMut,
    env: &Env,
    migrated_from: &Addr,
    admin: &Addr,
    nft_dossiers: Vec<BatchNftDossierElement>,
) -> StdResult<Vec<String>> {
    let mints = nft_dossiers
        .iter()
        .map(|nft| {
            let royalty_info = if let Some(some_royalty_info) = nft.royalty_info.clone() {
                Some(RoyaltyInfo {
                    decimal_places_in_rates: some_royalty_info.decimal_places_in_rates,
                    royalties: some_royalty_info
                        .royalties
                        .iter()
                        .map(|r| Royalty {
                            recipient: r.recipient.clone().unwrap().to_string(),
                            rate: r.rate,
                        })
                        .collect(),
                })
            } else {
                None
            };

            let mint_run_info = nft.mint_run_info.clone().unwrap();
            let serial_number = if let Some(some_serial_number) = mint_run_info.serial_number {
                Some(SerialNumber {
                    mint_run: mint_run_info.mint_run,
                    serial_number: some_serial_number,
                    quantity_minted_this_run: mint_run_info.quantity_minted_this_run,
                })
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
                transferable: Some(nft.transferable),
                memo: Some(format!("\"migrated_from\": \"{}\"", migrated_from)),
            }
        })
        .collect();
    let mut config: Config = load(deps.storage, CONFIG_KEY)?;
    let sender_raw = &deps.api.addr_canonicalize(admin.as_str())?;
    mint_list(deps, env, &mut config, sender_raw, mints)
}

pub(crate) fn migrate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract_mode: ContractMode,
    snip721config: &mut Config,
    admin_permit: Permit,
    migrate_to: MigrateTo,
) -> StdResult<Response> {
    check_contract_mode(vec![ContractMode::Running], &contract_mode, None)?;
    let admin_addr = &deps.api.addr_humanize(&snip721config.admin).unwrap();
    let permit_creator = &deps
        .api
        .addr_validate(&validate(
            deps.as_ref(),
            PREFIX_REVOKED_PERMITS,
            &admin_permit,
            env.contract.address.to_string(),
            Some("secret"),
        )?)
        .unwrap();

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

    let mut rng = ContractPrng::new(seed, &rng_entropy);

    let secret = Binary::from(rng.rand_bytes());

    let migrated_to = Some(MigratedToState {
        contract: CanonicalContractInfo {
            address: deps.api.addr_canonicalize(migrate_to_address.as_str())?,
            code_hash: migrate_to.code_hash,
        },
        migration_secret: secret.clone(),
    });
    MIGRATED_TO.save(deps.storage, &migrated_to.unwrap())?;
    CONTRACT_MODE.save(deps.storage, &ContractMode::MigrateOutStarted)?;

    let royalty_info: Option<RoyaltyInfo> =
        match may_load::<StoredRoyaltyInfo>(deps.storage, DEFAULT_ROYALTY_KEY)? {
            Some(stored_royalty_info) => {
                Some(stored_to_msg_royalty_info(stored_royalty_info, deps.api))
            }
            None => None,
        };

    Ok(Response::default().set_data(
        to_binary(&InstantiateByMigrationReplyDataMsg {
            migrated_instantiate_msg: Snip721InstantiateMsg {
                name: snip721config.name.to_string(),
                symbol: snip721config.symbol.to_string(),
                admin: Some(admin_addr.to_string()),
                entropy: entropy.to_string(),
                royalty_info,
                config: Some(InstantiateConfig {
                    public_token_supply: Some(snip721config.token_supply_is_public),
                    public_owner: Some(snip721config.owner_is_public),
                    enable_sealed_metadata: Some(snip721config.sealed_metadata_is_enabled),
                    unwrapped_metadata_is_private: Some(snip721config.unwrap_to_private),
                    minter_may_update_metadata: Some(snip721config.minter_may_update_metadata),
                    owner_may_update_metadata: Some(snip721config.owner_may_update_metadata),
                    enable_burn: Some(snip721config.burn_is_enabled),
                }),
                post_init_callback: None,
                post_init_data: None,
            },
            migrate_from: MigrateFrom {
                address: env.contract.address,
                code_hash: env.contract.code_hash,
                admin_permit,
            },
            remaining_migration_complete_event_sub_slots:
                REMAINING_MIGRATION_COMPLETE_EVENT_SUB_SLOTS.load(deps.storage)?,
            migration_complete_event_subscribers: MIGRATION_COMPLETE_EVENT_SUBSCRIBERS
                .may_load(deps.storage)?
                .map(|contracts| {
                    contracts
                        .into_iter()
                        .map(|c| c.into_humanized(deps.api).unwrap())
                        .collect()
                }),
            minters: load(deps.storage, MINTERS_KEY)?,
            mint_count: snip721config.mint_cnt,
            secret,
        })
        .unwrap(),
    ))
}

/// Returns StdResult<Binary(MigrationBatchNftDossier)> of all the token information for multiple tokens.
/// This can only be used by the contract being migrated to at migration_addr
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
/// * `block` - a reference to the BlockInfo
/// * `contract_mode` - a reference to this contract's contract mode state
/// * `start_index` - optionally only display token starting at this index
/// * `max_count` - optional max number of tokens to display
/// * `secret` - the migration secret
pub(crate) fn migration_dossier_list(
    deps: Deps,
    block: &BlockInfo,
    contract_mode: &ContractMode,
    start_index: Option<u32>,
    max_count: Option<u32>,
    secret: &Binary,
) -> StdResult<Binary> {
    check_contract_mode(vec![ContractMode::MigrateOutStarted], contract_mode, None)?;
    let migrated_to = MIGRATED_TO.may_load(deps.storage)?;
    if migrated_to.is_none() {
        return Err(StdError::generic_err(
            "This contract has not been migrated yet",
        ));
    }
    let migration_secret = &migrated_to.unwrap().migration_secret;
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
                let (inventory_approvals, view_owner_exp, view_meta_exp) = gen_snip721_approvals(
                    deps.api,
                    block,
                    &mut all_perm,
                    incl_exp,
                    &perm_type_info,
                )?;
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
                .map(|r| r.to_human(deps.api, false))
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
    to_binary(&MigrationBatchNftDossier {
        last_mint_index: idx,
        nft_dossiers: dossiers,
    })
}
