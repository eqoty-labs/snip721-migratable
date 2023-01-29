package io.eqoty.dapp.secret.types.contract

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject

fun Snip721Msgs.QueryAnswer.BatchNftDossier.equals(
    other: Snip721Msgs.QueryAnswer.BatchNftDossier,
    ignoreTimeOfMinting: Boolean
): Boolean {
    return if (ignoreTimeOfMinting) {
        val a = this.nftDossiers.map { it.asMapWithoutTimeOfMinting() }
        val b = other.nftDossiers.map { it.asMapWithoutTimeOfMinting() }
        a == b
    } else {
        this == other
    }
}

fun Snip721Msgs.QueryAnswer.NftDossier.asMapWithoutTimeOfMinting(): JsonObject {
    return JsonObject(Json.encodeToJsonElement(this).jsonObject.toMutableMap().apply {
        this["mint_run_info"] = JsonObject(this["mint_run_info"]!!.jsonObject.toMutableMap().apply {
            remove("time_of_minting")
        })
    })
}