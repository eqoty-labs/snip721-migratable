package io.eqoty.dapp.secret.types.contract

import io.eqoty.secretk.types.Coin
import io.eqoty.secretk.types.extensions.Permit
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

object PurchasableSnip721Msgs {
    @Serializable
    data class Instantiate(
        @SerialName("migrate_from")
        val migrateFrom: MigrateFrom? = null,
        val prices: List<Coin>? = null,
        @SerialName("public_metadata")
        val publicMetadata: Snip721Msgs.Metadata? = null,
        @SerialName("private_metadata")
        val privateMetadata: Snip721Msgs.Metadata? = null,
        val admin: String? = null,
        val entropy: String,
        @SerialName("royalty_info")
        val royaltyInfo: Snip721Msgs.RoyaltyInfo? = null,
    )

    @Serializable
    data class Execute(
        @SerialName("purchase_mint") val purchaseMint: PurchaseMint? = null,
    ) {
        @Serializable
        class PurchaseMint
    }

    @Serializable
    data class Query(
        @SerialName("get_prices") val getPrices: GetPrices? = null,
    ) {
        @Serializable
        class GetPrices
    }

    @Serializable
    data class QueryAnswer(
        @SerialName("get_prices") val getPrices: GetPrices? = null,
    ) {
        @Serializable
        data class GetPrices(val prices: List<Coin>)
    }

}

@Serializable
data class MigrateFrom(
    val address: String,
    @SerialName("code_hash")
    val codeHash: String,
    @SerialName("admin_permit")
    val adminPermit: Permit,
)