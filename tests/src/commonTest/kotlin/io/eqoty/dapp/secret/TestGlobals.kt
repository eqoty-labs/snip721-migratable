@file:Suppress("VARIABLE_IN_SINGLETON_WITHOUT_THREAD_LOCAL")

package io.eqoty.dapp.secret

import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.types.ContractInfo
import io.eqoty.dapp.secret.utils.NodeInfo
import io.eqoty.dapp.secret.utils.getNode
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.wallet.DirectSigningWallet
import kotlinx.coroutines.sync.Semaphore

/***
 * IntegrationTests will be re-instantiated for each test.
 * So this Global object holds properties that do not need to
 * be recreated each test.
 */
object TestGlobals {
    lateinit var client: SigningCosmWasmClient
    val testnetInfo: NodeInfo = getNode("src/commonTest/resources/config/nodes.json")

    // Returns a client with which we can interact with secret network
    suspend fun initializeClient(endpoint: String, chainId: String): SigningCosmWasmClient {
        val wallet = DirectSigningWallet() // Use default constructor of wallet to generate random mnemonic.
        val accAddress = wallet.accounts[0].address
        val client = SigningCosmWasmClient.init(
            endpoint,
            accAddress,
            wallet,
            chainId = chainId
        )

        Logger.i("Initialized client with wallet address: $accAddress")
        return client
    }

}
