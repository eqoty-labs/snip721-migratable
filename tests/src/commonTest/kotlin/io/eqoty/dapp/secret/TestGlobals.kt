@file:Suppress("VARIABLE_IN_SINGLETON_WITHOUT_THREAD_LOCAL")

package io.eqoty.dapp.secret

import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.types.ContractInfo
import io.eqoty.dapp.secret.utils.TestnetInfo
import io.eqoty.dapp.secret.utils.getTestnet
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
    lateinit var contractInfo: ContractInfo
    val testnetInfo: TestnetInfo = getTestnet()

    var needsInit = true
    val initTestsSemaphore = Semaphore(1)


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
