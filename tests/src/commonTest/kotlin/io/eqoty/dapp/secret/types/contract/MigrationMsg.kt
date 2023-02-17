package io.eqoty.dapp.secret.types.contract

import io.eqoty.secretk.types.extensions.Permit
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

object MigrationMsg {

    @Serializable
    data class InstantiateByMigration(
        @SerialName("migrate_from")
        val migrateFrom: MigrateFrom? = null,
        val entropy: String,
    )

    @Serializable
    data class MigrateFrom(
        val address: String,
        @SerialName("code_hash")
        val codeHash: String,
        @SerialName("admin_permit")
        val adminPermit: Permit,
    )

}