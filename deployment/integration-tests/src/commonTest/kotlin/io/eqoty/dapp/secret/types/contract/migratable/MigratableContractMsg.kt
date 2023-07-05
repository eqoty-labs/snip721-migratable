package io.eqoty.dapp.secret.types.contract.migratable

import kotlinx.serialization.Serializable

object MigratableContractMsg {

    @Serializable
    data class Instantiate(
        val migrate: MigratableContractTypes.InstantiateByMigration? = null,
    )

}

