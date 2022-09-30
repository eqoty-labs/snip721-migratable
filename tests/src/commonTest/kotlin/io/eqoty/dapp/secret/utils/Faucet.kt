package io.eqoty.dapp.secret.utils

import co.touchlab.kermit.Logger
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*

object Faucet {

    private val httpClient = HttpClient {}

    suspend fun fillUp(
        testnetInfo: TestnetInfo,
        client: SigningCosmWasmClient,
        targetBalance: Int
    ) {
        var balance = try {
            getScrtBalance(client)
        } catch (t: Throwable) {
            Logger.i(t.message ?: "getScrtBalance failed")
            Logger.i("Attempting to fill address ${client.senderAddress} from faucet")
            0
        }
        while (balance < targetBalance) {
            try {
                getFromFaucet(testnetInfo, client.wallet.getAccounts()[0].address)
            } catch (t: Throwable) {
                throw RuntimeException("failed to get tokens from faucet: $t")
            }
            var newBalance = balance
            val maxTries = 10
            var tries = 0
            while (balance == newBalance) {
                // the api doesn't update immediately. So retry until the balance changes
                newBalance = try {
                    getScrtBalance(client)
                } catch (t: Throwable) {
                    Logger.i("getScrtBalance try ${++tries}/$maxTries failed with: ${t.message}")
                    0
                }
                if (tries >= maxTries) {
                    throw RuntimeException("getScrtBalance did not update after $maxTries trys")
                }
            }
            balance = newBalance
            Logger.i("got tokens from faucet. New balance: $balance, target balance: $targetBalance")
        }
    }

    private suspend fun getScrtBalance(client: SigningCosmWasmClient): Int {
        val balance = client.getBalance(client.wallet.getAccounts()[0].address).balances
        return (balance.getOrNull(0)?.amount ?: "0").toInt()
    }

    private suspend fun getFromFaucet(testnetInfo: TestnetInfo, address: String): String {
        return when (testnetInfo) {
            is Pulsar2 -> {
                httpClient.post(testnetInfo.faucetAddressEndpoint) {
                    setBody(
                        """
                            {
                                "denom": "uscrt",
                                "address": $address
                            }
                        """
                    )
                }.bodyAsText()
            }

            else -> {
                httpClient.get(testnetInfo.createFaucetAddressGetEndpoint(address)).bodyAsText()
            }
        }
    }

}
