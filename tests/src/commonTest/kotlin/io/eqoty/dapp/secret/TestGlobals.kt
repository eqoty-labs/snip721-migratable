@file:Suppress("VARIABLE_IN_SINGLETON_WITHOUT_THREAD_LOCAL")

package io.eqoty.dapp.secret

import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.utils.NodeInfo
import io.eqoty.dapp.secret.utils.getNode
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.wallet.DirectSigningWallet

/***
 * IntegrationTests will be re-instantiated for each test.
 * So this Global object holds properties that do not need to
 * be recreated each test.
 */
object TestGlobals {
    private var clientBacking: SigningCosmWasmClient? = null
    val client: SigningCosmWasmClient get() = clientBacking!!
    val clientInitialized = clientBacking != null

    val testnetInfo: NodeInfo = getNode("src/commonTest/resources/config/nodes.json")

    // Returns a client with which we can interact with secret network
    suspend fun initializeClient(endpoint: String, chainId: String, numberOfWalletAccounts: Int) {
        val wallet = DirectSigningWallet() // Use default constructor of wallet to generate random mnemonic.
        (1 until numberOfWalletAccounts).forEach {
            wallet.addAccount()
        }
        val accAddress = wallet.accounts[0].address
        val client = SigningCosmWasmClient.init(
            endpoint,
            accAddress,
            wallet,
            chainId = chainId
        )

        Logger.i("Initialized client with wallet accounts: ${wallet.accounts.map { it.address }}")
        clientBacking = client
    }

}
