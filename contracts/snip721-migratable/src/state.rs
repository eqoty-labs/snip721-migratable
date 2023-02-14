use serde::{Deserialize, Serialize};

/// key for MigrateInTokensProgress singleton
pub static MIGRATE_IN_TOKENS_PROGRESS_KEY: &[u8] = b"migrateintknsprogress";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MigrateInTokensProgress {
    /// the mint_cnt of the contract being migrated from
    pub migrate_in_mint_cnt: u32,
    /// the next mint index out of migrate_in_mint_cnt that must be migrated
    pub migrate_in_next_mint_index: u32,
}

