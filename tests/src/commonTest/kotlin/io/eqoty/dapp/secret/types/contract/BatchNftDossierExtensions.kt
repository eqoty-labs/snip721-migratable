package io.eqoty.dapp.secret.types.contract

import io.eqoty.secret.std.contract.msg.Snip721Msgs
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject

fun Snip721Msgs.QueryAnswer.BatchNftDossier.equals(
    other: Snip721Msgs.QueryAnswer.BatchNftDossier,
    ignoreCollectionCreator: Boolean,
    ignoreTokenCreator: Boolean,
    ignoreTimeOfMinting: Boolean,
): Boolean {
    return if (ignoreCollectionCreator || ignoreTokenCreator || ignoreTimeOfMinting) {
        val a = this.nftDossiers.map { it.asMap(ignoreCollectionCreator, ignoreTokenCreator, ignoreTimeOfMinting) }
        val b = other.nftDossiers.map { it.asMap(ignoreCollectionCreator, ignoreTokenCreator, ignoreTimeOfMinting) }
        a == b
    } else {
        this == other
    }
}

fun Snip721Msgs.QueryAnswer.NftDossier.asMap(
    removeCollectionCreator: Boolean,
    removeTokenCreator: Boolean,
    removeTimeOfMinting: Boolean,
): JsonObject {
    return JsonObject(Json.encodeToJsonElement(this).jsonObject.toMutableMap().apply {
        this["mint_run_info"] = JsonObject(this["mint_run_info"]!!.jsonObject.toMutableMap().apply {
            if (removeCollectionCreator) remove("collection_creator")
            if (removeTokenCreator) remove("token_creator")
            if (removeTimeOfMinting) remove("time_of_minting")
        })
    })
}