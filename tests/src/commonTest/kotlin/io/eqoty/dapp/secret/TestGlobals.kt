@file:Suppress("VARIABLE_IN_SINGLETON_WITHOUT_THREAD_LOCAL")

package io.eqoty.dapp.secret

import co.touchlab.kermit.Logger
import io.eqoty.cosmwasm.std.types.Coin
import io.eqoty.cosmwasm.std.types.ContractInfo
import io.eqoty.dapp.secret.utils.NodeInfo
import io.eqoty.dapp.secret.utils.getNode
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.types.MsgSend
import io.eqoty.secretk.types.TxOptions
import io.eqoty.secretk.wallet.DirectSigningWallet

/***
 * IntegrationTests will be re-instantiated for each test.
 * So this Global object holds properties that do not need to
 * be recreated each test.
 */
object TestGlobals {
    private var clientBacking: SigningCosmWasmClient? = null
    val client: SigningCosmWasmClient get() = clientBacking!!
    val clientInitialized get() = clientBacking != null

    val testnetInfo: NodeInfo = getNode("src/commonTest/resources/config/nodes.json")

    // Returns a client with which we can interact with secret network
    suspend fun initializeClient(endpoint: String, chainId: String, numberOfWalletAccounts: Int) {
        val wallet = DirectSigningWallet() // Use default constructor of wallet to generate random mnemonic.
        (1 until numberOfWalletAccounts).forEach {
            wallet.addAccount()
        }
        wallet.addressToAccountSigningData.values.forEach { a ->
            Logger.i("Added random account to wallet w/ mnemonic: ${a.mnemonic?.map { it.concatToString() }}")
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

    suspend fun intializeAccountBeforeExecuteWorkaround(senderAddress: String) {
        // workaround for weird issue where you need to execute a tx once (where it errors) before execute or
        // simulate can be called successfully on a brand-new account:
        // https://discord.com/channels/360051864110235648/603225118545674241/1030724640315805716
        val msgs = listOf(
            MsgSend(
                fromAddress = senderAddress,
                toAddress = senderAddress,
                amount = listOf(Coin(1, "usrct")),
            )
        )
        val originalSenderAddress = client.senderAddress
        try {
            client.senderAddress = senderAddress
            client.execute(
                msgs,
                txOptions = TxOptions(gasLimit = 100000)
            )
        } catch (_: Throwable) {
        }
        client.senderAddress = originalSenderAddress
    }
}
