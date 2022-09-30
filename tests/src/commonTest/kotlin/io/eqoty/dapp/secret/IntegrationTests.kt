package io.eqoty.dapp.secret

import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.TestGlobals.client
import io.eqoty.dapp.secret.TestGlobals.contractInfo
import io.eqoty.dapp.secret.TestGlobals.initTestsSemaphore
import io.eqoty.dapp.secret.TestGlobals.initializeClient
import io.eqoty.dapp.secret.TestGlobals.needsInit
import io.eqoty.dapp.secret.TestGlobals.testnetInfo
import io.eqoty.dapp.secret.types.ContractInfo
import io.eqoty.dapp.secret.types.contract.CountResponse
import io.eqoty.dapp.secret.utils.Faucet
import io.eqoty.dapp.secret.utils.fileSystem
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.types.MsgExecuteContract
import io.eqoty.secretk.types.MsgInstantiateContract
import io.eqoty.secretk.types.MsgStoreCode
import io.eqoty.secretk.types.TxOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import okio.Path
import okio.Path.Companion.toPath
import kotlin.math.ceil
import kotlin.random.Random
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals

class IntegrationTests {

    private val contractCodePath: Path = "../contract.wasm.gz".toPath()

    // Initialization procedure
    private suspend fun initializeAndUploadContract() {
        val endpoint = testnetInfo.grpcGatewayEndpoint

        client = initializeClient(endpoint, testnetInfo.chainId)

        Faucet.fillUp(testnetInfo, client, 100_000_000)

        val initMsg = """{"count": 4}"""
        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = client.senderAddress,
                codeId = null, // will be set later
                initMsg = initMsg,
                label = "My Snip721" + ceil(Random.nextDouble() * 10000),
                codeHash = null // will be set later
            )
        )
        contractInfo = storeCodeAndInstantiate(
            client,
            contractCodePath,
            instantiateMsgs
        )
    }

    private suspend fun storeCodeAndInstantiate(
        client: SigningCosmWasmClient,
        codePath: Path,
        instantiateMsgs: List<MsgInstantiateContract>
    ): ContractInfo {
        val accAddress = client.wallet.getAccounts()[0].address
        val wasmBytes =
            fileSystem.read(codePath) {
                readByteArray()
            }

        val msgs0 = listOf(
            MsgStoreCode(
                sender = accAddress,
                wasmByteCode = wasmBytes.toUByteArray(),
            )
        )
        var simulate = client.simulate(msgs0)
        var gasLimit = (simulate.gasUsed.toDouble() * 1.1).toInt()
        val response = client.execute(
            msgs0,
            txOptions = TxOptions(gasLimit = gasLimit)
        )

        val codeId = response.logs[0].events
            .find { it.type == "message" }
            ?.attributes
            ?.find { it.key == "code_id" }?.value!!
        Logger.i("codeId:  $codeId")

        val codeInfo = client.getCodeInfoByCodeId(codeId)
        Logger.i("code hash: ${codeInfo.codeHash}")

        val codeHash = codeInfo.codeHash

        instantiateMsgs.forEach {
            it.codeId = codeId.toInt()
            it.codeHash = codeHash
        }
        simulate = client.simulate(instantiateMsgs)
        gasLimit = (simulate.gasUsed.toDouble() * 1.1).toInt()
        val instantiateResponse = client.execute(
            instantiateMsgs,
            txOptions = TxOptions(gasLimit = gasLimit)
        )
        val contractAddress = instantiateResponse.logs[0].events
            .find { it.type == "message" }
            ?.attributes
            ?.find { it.key == "contract_address" }?.value!!
        Logger.i("contract address:  $contractAddress")
        return ContractInfo(codeHash, contractAddress)
    }

    private suspend fun queryCount(): CountResponse {
        val contractInfoQuery = """{"get_count": {}}"""
        return Json.decodeFromString(
            client.queryContractSmart(
                contractInfo.contractAddress,
                contractInfoQuery
            )
        )
    }

    private suspend fun incrementTx(
        contractInfo: ContractInfo
    ) {
        val incrementMsg = """{"increment": {}}"""

        val msgs1 = listOf(
            MsgExecuteContract(
                sender = client.senderAddress,
                contractAddress = contractInfo.contractAddress,
                codeHash = contractInfo.codeHash,
                msg = incrementMsg,
            )
        )
        val gasLimit = 200000
        val result = client.execute(
            msgs1,
            txOptions = TxOptions(gasLimit = gasLimit)
        )
        Logger.i("Increment TX used ${result.gasUsed}")
    }


    @BeforeTest
    fun beforeEach() = runTest {
        withContext(Dispatchers.IO) {
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

    }

    @Test
    fun test_count_on_initialization() = runTest {
        val countResponse = queryCount()
        Logger.i("Count Response: $countResponse")
        assertEquals(4, countResponse.count)
    }

    @Test
    fun test_increment_stress() = runTest {
        val onStartCounter = queryCount().count

        val stressLoad = 10
        for (i in 0 until 10) {
            incrementTx(contractInfo)
        }

        val afterStressCounter = queryCount().count
        assertEquals(
            stressLoad, afterStressCounter - onStartCounter,
            "After running stress test the counter expected to be ${onStartCounter + 10} instead " +
                    "of ${afterStressCounter}"
        )
    }

}
