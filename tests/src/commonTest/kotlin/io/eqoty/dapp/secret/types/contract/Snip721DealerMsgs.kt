package io.eqoty.dapp.secret.types.contract

import io.eqoty.secretk.types.Coin
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

object Snip721DealerMsgs {
    @Serializable
    data class Instantiate(
        val migrate: MigrationMsg.InstantiateByMigration? = null,
        val new: InstantiateSelfAnChildSnip721Msg? = null,
    ) {

        @Serializable
        data class InstantiateSelfAnChildSnip721Msg(
            @SerialName("snip721_code_info")
            val snip721CodeInfo: CosmWasmStd.CodeInfo,
            @SerialName("snip721_label")
            val snip721Label: String,
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

    }

    @Serializable
    data class Execute(
        @SerialName("purchase_mint") val purchaseMint: PurchaseMint? = null,
        @SerialName("migrate_tokens_in") val migrateTokensIn: MigrateTokensIn? = null,
    ) {
        @Serializable
        class PurchaseMint

        @Serializable
        data class MigrateTokensIn(
            val pages: UInt? = null,
            @SerialName("page_size")
            val pageSize: UInt? = null,
        )
    }

    @Serializable
    data class Query(
        @SerialName("get_prices") val getPrices: GetPrices? = null,
        @SerialName("get_child_snip721") val getChildSnip721: GetChildSnip721? = null,
        @SerialName("migrated_from") val migratedFrom: MigratedFrom? = null,
        @SerialName("migrated_to") val migratedTo: MigratedTo? = null,
    ) {
        @Serializable
        class GetPrices

        @Serializable
        class MigratedFrom

        @Serializable
        class MigratedTo

        @Serializable
        class GetChildSnip721
    }

    @Serializable
    data class QueryAnswer(
        @SerialName("get_prices") val getPrices: GetPrices? = null,
        @SerialName("migration_info") val migrationInfo: CosmWasmStd.ContractInfo? = null,
        @SerialName("contract_info") val contractInfo: CosmWasmStd.ContractInfo? = null,
    ) {
        @Serializable
        data class GetPrices(val prices: List<Coin>)

    }

    @Serializable
    data class ExecuteAnswer(
        @SerialName("migrate_tokens_in") val migrateTokensIn: Execute.MigrateTokensIn? = null,
    ) {
        @Serializable
        data class MigrateTokensIn(
            val complete: Boolean,
            @SerialName("next_mint_index")
            val nextMintIndex: UInt?,
            val total: UInt?,
        )
    }

}
