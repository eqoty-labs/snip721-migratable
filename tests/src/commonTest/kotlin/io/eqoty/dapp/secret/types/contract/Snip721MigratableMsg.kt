package io.eqoty.dapp.secret.types.contract

import kotlinx.serialization.Serializable

object Snip721MigratableMsg {

    @Serializable
    data class Instantiate(
        val migrate: MigrationMsg.InstantiateByMigration? = null,
    )

}