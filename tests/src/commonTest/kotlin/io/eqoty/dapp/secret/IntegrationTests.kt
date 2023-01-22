package io.eqoty.dapp.secret

import DeployContractUtils
import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.TestGlobals.client
import io.eqoty.dapp.secret.TestGlobals.contractInfo
import io.eqoty.dapp.secret.TestGlobals.initTestsSemaphore
import io.eqoty.dapp.secret.TestGlobals.initializeClient
import io.eqoty.dapp.secret.TestGlobals.needsInit
import io.eqoty.dapp.secret.TestGlobals.testnetInfo
import io.eqoty.dapp.secret.types.ContractInfo
import io.eqoty.dapp.secret.types.ExecuteResult
import io.eqoty.dapp.secret.types.MintedRelease
import io.eqoty.dapp.secret.types.contract.EqotyPurchaseMsgs
import io.eqoty.dapp.secret.types.contract.PurchasableSnip721Msgs
import io.eqoty.dapp.secret.types.contract.Snip721Msgs
import io.eqoty.dapp.secret.utils.*
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.types.*
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okio.Path
import okio.Path.Companion.toPath
import kotlin.math.ceil
import kotlin.random.Random
import kotlin.test.BeforeTest
import kotlin.test.Test

class IntegrationTests {

    private val contractCodePath: Path = getEnv(Constants.CONTRACT_PATH_ENV_NAME)!!.toPath()
    private val purchasePrices = listOf(Coin(amount = 2000000, denom = "uscrt"))

    // Initialization procedure
    private suspend fun initializeAndUploadContract() {
        val endpoint = testnetInfo.grpcGatewayEndpoint

        client = initializeClient(endpoint, testnetInfo.chainId)

        BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000)

        val initMsg = PurchasableSnip721Msgs.Instantiate(
            prices = purchasePrices,
            publicMetadata = Snip721Msgs.Metadata("publicMetadataUri"),
            privateMetadata = Snip721Msgs.Metadata("privateMetadataUri"),
            admin = client.senderAddress,
            entropy = "sometimes you gotta close a door to open a window"
        )
        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = client.senderAddress,
                codeId = null, // will be set later
                initMsg = Json.encodeToString(initMsg),
                label = "My Snip721" + ceil(Random.nextDouble() * 10000),
                codeHash = null // will be set later
            )
        )
        contractInfo = DeployContractUtils.storeCodeAndInstantiate(
            client,
            contractCodePath,
            instantiateMsgs,
        )
    }

    private suspend fun purchaseOneMint(
        client: SigningCosmWasmClient,
        contractInfo: ContractInfo,
        sentFunds: List<Coin>
    ): ExecuteResult<MintedRelease> {
        val purchaseMintMsg = Json.encodeToString(
            EqotyPurchaseMsgs.Execute(
                purchaseMint = EqotyPurchaseMsgs.Execute.PurchaseMint()
            )
        )
        val msgs = listOf(
            MsgExecuteContract(
                sender = client.senderAddress,
                contractAddress = contractInfo.address,
                codeHash = contractInfo.codeInfo.codeHash,
                msg = purchaseMintMsg,
                sentFunds = sentFunds
            )
        )
        val gasLimit = try {
            val simulate = client.simulate(msgs)
            (simulate.gasUsed.toDouble() * 1.1).toInt()
        } catch (_: Throwable) {
            200_000
        }
        val txOptions = TxOptions(gasLimit = gasLimit)
        val res = try {
            client.execute(
                msgs,
                txOptions = txOptions
            )
        } catch (t: Throwable) {
            Logger.i(t.message ?: "")
            null
        }
        val gasFee = client.gasToFee(txOptions.gasLimit, txOptions.gasPriceInFeeDenom)
        return ExecuteResult(res, Coin(gasFee, "uscrt"))
    }


    @BeforeTest
    fun beforeEach() = runTest {
        initTestsSemaphore.acquire()
        try {
            if (needsInit) {
                Logger.setTag("dapp")
                initializeAndUploadContract()
                needsInit = false
            }
        } catch (t: Throwable) {
            throw t
        } finally {
            initTestsSemaphore.release()
        }
    }

    @Test
    fun test_count_on_initialization() = runTest {
        val purchaseOneMintResult =
            purchaseOneMint(client, contractInfo, purchasePrices)

    }


}
