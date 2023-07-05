package io.eqoty.dapp.secret.types

import io.eqoty.cosmwasm.std.types.Coin
import io.eqoty.secretk.types.response.TxResponseData

data class ExecuteResult<T>(val response: TxResponseData?, val gasFee: Coin, val info: T? = null)
