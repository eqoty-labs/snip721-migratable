use cosmwasm_std::{Binary, CanonicalAddr, ContractInfo, Deps, DepsMut, Env, from_binary, MessageInfo, Reply, Response, StdError, StdResult, SubMsg, to_binary, WasmMsg};
use secret_toolkit::crypto::Prng;
use secret_toolkit::permit::{Permit, validate};
use secret_toolkit::viewing_key::{ViewingKey, ViewingKeyStore};
use snip721_reference_impl::state::{load, may_load, PREFIX_REVOKED_PERMITS, save};

use migration::msg::{MigratableQueryAnswer, MigrationListenerExecuteMsg};
use migration::msg_types::{MigrateFrom, MigrateTo};
use migration::state::{ContractMode, MIGRATED_FROM_KEY, MIGRATED_TO_KEY, MigratedFromState, MigratedToState, NOTIFY_ON_MIGRATION_COMPLETE_KEY};

use crate::msg::{DealerState, InstantiateByMigrationReplyDataMsg};
use crate::state::{ADMIN_KEY, CHILD_SNIP721_ADDRESS_KEY, CHILD_SNIP721_CODE_HASH_KEY, CONTRACT_MODE_KEY, PURCHASABLE_METADATA_KEY, PurchasableMetadata, PURCHASE_PRICES_KEY};

pub(crate) fn instantiate_with_migrated_config(deps: DepsMut, msg: Reply) -> StdResult<Response> {
    let reply_data: InstantiateByMigrationReplyDataMsg = from_binary(&msg.result.unwrap().data.unwrap()).unwrap();

    let migrated_from = MigratedFromState {
        contract: ContractInfo {
            address: deps.api.addr_validate(reply_data.migrate_from.address.as_str()).unwrap(),
            code_hash: reply_data.migrate_from.code_hash,
        },
        migration_secret: reply_data.secret,
    };
    save(deps.storage, ADMIN_KEY, &deps.api.addr_canonicalize(&reply_data.dealer_state.admin.as_str())?)?;
    save(deps.storage, PURCHASE_PRICES_KEY, &reply_data.dealer_state.prices)?;
    save(deps.storage, CHILD_SNIP721_CODE_HASH_KEY, &reply_data.dealer_state.child_snip721_code_hash)?;
    save(deps.storage, CHILD_SNIP721_ADDRESS_KEY, &deps.api.addr_canonicalize(reply_data.dealer_state.child_snip721_address.as_str())?)?;
    save(deps.storage, MIGRATED_FROM_KEY, &migrated_from)?;
    save(deps.storage, PURCHASABLE_METADATA_KEY, &PurchasableMetadata {
        public_metadata: reply_data.dealer_state.public_metadata,
        private_metadata: reply_data.dealer_state.private_metadata,
    })?;
    save(deps.storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY, &reply_data.on_migration_complete_notify_receiver)?;

    save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::Running)?;


    // clear the data (that contains the secret) which would be set when init_snip721 is called
    // from reply as part of the migration process
    // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
    return Ok(Response::default().set_data(b""));
}


pub(crate) fn migrate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    admin_permit: Permit,
    migrate_to: MigrateTo,
) -> StdResult<Response> {
    let admin_addr = &deps.api.addr_humanize(&load::<CanonicalAddr>(deps.storage, ADMIN_KEY)?)?;
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
    let migrate_to_address = deps.api.addr_validate(migrate_to.address.as_str())?;
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
    save(deps.storage, CONTRACT_MODE_KEY, &ContractMode::MigratedOut)?;

    let purchasable_metadata: PurchasableMetadata = load(deps.storage, PURCHASABLE_METADATA_KEY)?;
    let child_snip721_code_hash: String = load(deps.storage, CHILD_SNIP721_CODE_HASH_KEY)?;
    let child_snip721_address: CanonicalAddr = load(deps.storage, CHILD_SNIP721_ADDRESS_KEY)?;
    let contracts = load::<Vec<ContractInfo>>(deps.storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY)?;
    let msg = to_binary(&MigrationListenerExecuteMsg::MigrationCompleteNotification {
        from: env.contract.clone().into()
    })?;
    let sub_msgs: Vec<SubMsg> = contracts.iter().map(|contract| {
        let execute = WasmMsg::Execute {
            msg: msg.clone(),
            contract_addr: contract.address.to_string(),
            code_hash: contract.code_hash.clone(),
            funds: vec![],
        };
        SubMsg::new(execute)
    }).collect();

    Ok(Response::default()
        .add_submessages(sub_msgs)
        .set_data(to_binary(&InstantiateByMigrationReplyDataMsg {
            dealer_state: DealerState {
                prices: load(deps.storage, PURCHASE_PRICES_KEY)?,
                public_metadata: purchasable_metadata.public_metadata,
                private_metadata: purchasable_metadata.private_metadata,
                admin: admin_addr.clone(),
                child_snip721_code_hash,
                child_snip721_address: deps.api.addr_humanize(&child_snip721_address)?,
            },
            migrate_from: MigrateFrom {
                address: env.contract.address,
                code_hash: env.contract.code_hash,
                admin_permit,
            },
            on_migration_complete_notify_receiver: load(deps.storage, NOTIFY_ON_MIGRATION_COMPLETE_KEY)?,
            secret,
        })?)
    )
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
            let migrated_from: Option<MigratedFromState> = may_load(deps.storage, MIGRATED_FROM_KEY)?;
            match migrated_from {
                None => {
                    to_binary(&MigratableQueryAnswer::MigrationInfo(None))
                }
                Some(some_migrated_from) => {
                    to_binary(&MigratableQueryAnswer::MigrationInfo(
                        Some(some_migrated_from.contract.into()))
                    )
                }
            }
        }
        false => {
            let migrated_to: Option<MigratedToState> = may_load(deps.storage, MIGRATED_TO_KEY)?;
            match migrated_to {
                None => {
                    to_binary(&MigratableQueryAnswer::MigrationInfo(None))
                }
                Some(some_migrated_to) => {
                    to_binary(&MigratableQueryAnswer::MigrationInfo(
                        Some(some_migrated_to.contract.into()))
                    )
                }
            }
        }
    };
}
