use cosmwasm_contract_migratable_std::msg::MigratableQueryAnswer::MigrationInfo;
use cosmwasm_contract_migratable_std::msg::MigrationListenerExecuteMsg;
use cosmwasm_contract_migratable_std::msg_types::{MigrateFrom, MigrateTo};
use cosmwasm_contract_migratable_std::state::{
    ContractMode, MigratedFromState, MigratedToState, CONTRACT_MODE_KEY, MIGRATED_FROM_KEY,
    MIGRATED_TO_KEY, NOTIFY_ON_MIGRATION_COMPLETE_KEY,
};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Api, Binary, BlockInfo, CanonicalAddr, ContractInfo, Deps,
    DepsMut, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, WasmMsg,
};
use cosmwasm_storage::ReadonlyPrefixedStorage;
use secret_toolkit::crypto::Prng;
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
use crate::msg::{
    ExecuteAnswer, ExecuteMsg, ExecuteMsgExt, InstantiateByMigrationReplyDataMsg, QueryAnswer,
    QueryMsgExt,
};
use crate::state::{MigrateInTokensProgress, MIGRATE_IN_TOKENS_PROGRESS_KEY};

pub(crate) fn instantiate_with_migrated_config(
    deps: DepsMut,
    env: &Env,
    msg: Reply,
) -> StdResult<Response> {
    let mut deps = deps;
    deps.api
        .debug(&*format!("msg.result: {:?}!", msg.result.clone().unwrap()));

    let reply_data: InstantiateByMigrationReplyDataMsg =
        from_binary(&msg.result.unwrap().data.unwrap()).unwrap();
    // admin of the contract being migrated should always be the sender here
    let admin_info = MessageInfo {
        sender: deps
            .api
            .addr_validate(
                reply_data
                    .migrated_instantiate_msg
                    .admin
                    .clone()
                    .unwrap()
                    .as_str(),
            )
            .unwrap(),
        funds: vec![],
    };

    // actually instantiate the snip721 base contract using the migrated data
    let snip721_response = init_snip721(
        &mut deps,
        env,
        admin_info.clone(),
        reply_data.migrated_instantiate_msg,
    )
    .unwrap();

    let migrated_from = MigratedFromState {
        contract: ContractInfo {
            address: deps
                .api
                .addr_validate(reply_data.migrate_from.address.as_str())
                .unwrap(),
            code_hash: reply_data.migrate_from.code_hash,
        },
        migration_secret: reply_data.secret,
    };
    save(deps.storage, MIGRATED_FROM_KEY, &migrated_from)?;
    let migrate_in_tokens_progress = MigrateInTokensProgress {
        migrate_in_mint_cnt: reply_data.mint_count,
        migrate_in_next_mint_index: 0,
    };
    save(
        deps.storage,
        MIGRATE_IN_TOKENS_PROGRESS_KEY,
        &migrate_in_tokens_progress,
    )?;
    save(deps.storage, MINTERS_KEY, &reply_data.minters)?;

    save(
        deps.storage,
        CONTRACT_MODE_KEY,
        &ContractMode::MigrateDataIn,
    )?;
    if let Some(on_migration_complete_notify_receiver) =
        reply_data.on_migration_complete_notify_receiver
    {
        save(
            deps.storage,
            NOTIFY_ON_MIGRATION_COMPLETE_KEY,
            &on_migration_complete_notify_receiver,
        )?;
    }

    // clear the data (that contains the secret) which would be set when init_snip721 is called
    // from reply as part of the migration process
    // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
    return Ok(snip721_response.set_data(b""));
}

pub(crate) fn perform_token_migration(
    deps: DepsMut,
    env: &Env,
    info: MessageInfo,
    snip721_config: Config,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    let migrated_from: MigratedFromState = load(deps.storage, MIGRATED_FROM_KEY)?;
    let admin_addr = deps.api.addr_humanize(&snip721_config.admin).unwrap();
    if admin_addr != info.sender {
        return Err(StdError::generic_err(format!(
            "This contract's admin must complete migrating contract data from {:?}",
            migrated_from.contract.address
        )));
    }

    let (pages, page_size) = match msg {
        ExecuteMsg::Ext(ext_msg) => match ext_msg {
            ExecuteMsgExt::MigrateTokensIn { pages, page_size } => {
                (pages.unwrap_or(u32::MAX), page_size)
            }
        },
        _ => {
            return Err(StdError::generic_err(format!(
                "This contract's admin must complete migrating contract data from {:?}",
                migrated_from.contract.address
            )));
        }
    };
    let mut deps = deps;
    let mut migrate_in_tokens_progress: MigrateInTokensProgress =
        load(deps.storage, MIGRATE_IN_TOKENS_PROGRESS_KEY)?;
    let mut start_at_idx = migrate_in_tokens_progress.migrate_in_next_mint_index;
    let mint_count = migrate_in_tokens_progress.migrate_in_mint_cnt;

    let mut pages_queried = 0;
    while pages_queried < pages && start_at_idx < mint_count {
        let query_answer: QueryAnswer = deps
            .querier
            .query_wasm_smart(
                migrated_from.contract.code_hash.clone(),
                migrated_from.contract.address.clone(),
                &QueryMsgExt::ExportMigrationData {
                    start_index: Some(start_at_idx),
                    max_count: page_size,
                    secret: migrated_from.migration_secret.clone(),
                },
            )
            .unwrap();
        pages_queried += 1;
        start_at_idx = match query_answer {
            MigrationBatchNftDossier {
                last_mint_index,
                nft_dossiers,
            } => {
                save_migration_dossier_list(
                    &mut deps,
                    env,
                    &migrated_from.contract.address.clone(),
                    &admin_addr,
                    nft_dossiers,
                )
                .unwrap();
                last_mint_index + 1
            }
        };
    }
    migrate_in_tokens_progress.migrate_in_next_mint_index = start_at_idx;
    save(
        deps.storage,
        MIGRATE_IN_TOKENS_PROGRESS_KEY,
        &migrate_in_tokens_progress,
    )?;

    return if start_at_idx < mint_count {
        Ok(
            Response::new().set_data(to_binary(&ExecuteAnswer::MigrateTokensIn {
                complete: false,
                next_mint_index: Some(start_at_idx),
                total: Some(mint_count),
            })?),
        )
    } else {
        // migration complete
        save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::Running)?;
        // always notify the contract being migrated from so it can change its mode from MigrateOutStarted to MigratedOut
        let contracts = &mut vec![migrated_from.contract.clone()];
        contracts.append(
            &mut may_load::<Vec<ContractInfo>>(deps.storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY)?
                .unwrap_or_default(),
        );
        let msg = to_binary(
            &MigrationListenerExecuteMsg::MigrationCompleteNotification {
                from: migrated_from.contract.into(),
            },
        )?;
        let sub_msgs: Vec<SubMsg> = contracts
            .iter()
            .map(|contract| {
                let execute = WasmMsg::Execute {
                    msg: msg.clone(),
                    contract_addr: contract.address.to_string(),
                    code_hash: contract.code_hash.clone(),
                    funds: vec![],
                };
                SubMsg::new(execute)
            })
            .collect();

        Ok(Response::new()
            .add_submessages(sub_msgs)
            .set_data(to_binary(&ExecuteAnswer::MigrateTokensIn {
                complete: true,
                next_mint_index: None,
                total: None,
            })?))
    };
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
                transferable: Some(nft.transferable.clone()),
                memo: Some(format!("Migrated from: {}", migrated_from)),
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
    snip721config: &mut Config,
    admin_permit: Permit,
    migrate_to: MigrateTo,
) -> StdResult<Response> {
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
    let mut migrated_to: Option<MigratedToState> = may_load(deps.storage, MIGRATED_TO_KEY)?;
    if migrated_to.is_some() {
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

    migrated_to = Some(MigratedToState {
        contract: ContractInfo {
            address: migrate_to_address,
            code_hash: migrate_to.code_hash,
        },
        migration_secret: secret.clone(),
    });
    save(deps.storage, MIGRATED_TO_KEY, &migrated_to.unwrap())?;
    save(
        deps.storage,
        CONTRACT_MODE_KEY,
        &ContractMode::MigrateOutStarted,
    )?;

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
            },
            migrate_from: MigrateFrom {
                address: env.contract.address,
                code_hash: env.contract.code_hash,
                admin_permit,
            },
            on_migration_complete_notify_receiver: may_load(
                deps.storage,
                NOTIFY_ON_MIGRATION_COMPLETE_KEY,
            )?,
            minters: load(deps.storage, MINTERS_KEY)?,
            mint_count: snip721config.mint_cnt,
            secret,
        })
        .unwrap(),
    ))
}

/// Returns StdResult<Binary> displaying the Migrated to/from contract info
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
/// * `migrated_from` - if migrated_from is true query returns info about the contract it was migrated
/// from otherwise if returns info about the info the contract was migrated to
pub(crate) fn query_migrated_info(deps: Deps, migrated_from: bool) -> StdResult<Binary> {
    return match migrated_from {
        true => {
            let migrated_from: Option<MigratedFromState> =
                may_load(deps.storage, MIGRATED_FROM_KEY)?;
            match migrated_from {
                None => to_binary(&MigrationInfo(None)),
                Some(some_migrated_from) => {
                    to_binary(&MigrationInfo(Some(some_migrated_from.contract.into())))
                }
            }
        }
        false => {
            let migrated_to: Option<MigratedToState> = may_load(deps.storage, MIGRATED_TO_KEY)?;
            match migrated_to {
                None => to_binary(&MigrationInfo(None)),
                Some(some_migrated_to) => {
                    to_binary(&MigrationInfo(Some(some_migrated_to.contract.into())))
                }
            }
        }
    };
}

/// Returns StdResult<Binary(MigrationBatchNftDossier)> of all the token information for multiple tokens.
/// This can only be used by the contract being migrated to at migration_addr
///
/// # Arguments
///
/// * `deps` - a reference to Extern containing all the contract's external dependencies
/// * `block` - a reference to the BlockInfo
/// * `state` - a reference to this contracts persisted state
/// * `start_index` - optionally only display token starting at this index
/// * `max_count` - optional max number of tokens to display
/// * `secret` - the migration secret
pub(crate) fn migration_dossier_list(
    deps: Deps,
    block: &BlockInfo,
    start_index: Option<u32>,
    max_count: Option<u32>,
    secret: &Binary,
) -> StdResult<Binary> {
    let migrated_to: Option<MigratedToState> = may_load(deps.storage, MIGRATED_TO_KEY)?;
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
