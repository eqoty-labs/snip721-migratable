package io.eqoty.dapp.secret

import DeployContractUtils
import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.TestGlobals.client
import io.eqoty.dapp.secret.TestGlobals.clientInitialized
import io.eqoty.dapp.secret.TestGlobals.initializeClient
import io.eqoty.dapp.secret.TestGlobals.testnetInfo
import io.eqoty.dapp.secret.types.ContractInfo
import io.eqoty.dapp.secret.types.ExecuteResult
import io.eqoty.dapp.secret.types.MintedRelease
import io.eqoty.dapp.secret.types.contract.EqotyPurchaseMsgs
import io.eqoty.dapp.secret.types.contract.MigrateFrom
import io.eqoty.dapp.secret.types.contract.PurchasableSnip721Msgs
import io.eqoty.dapp.secret.types.contract.Snip721Msgs
import io.eqoty.dapp.secret.utils.BalanceUtils
import io.eqoty.dapp.secret.utils.Constants
import io.eqoty.dapp.secret.utils.getEnv
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.extensions.accesscontrol.PermitFactory
import io.eqoty.secretk.types.Coin
import io.eqoty.secretk.types.MsgExecuteContract
import io.eqoty.secretk.types.MsgInstantiateContract
import io.eqoty.secretk.types.TxOptions
import io.eqoty.secretk.types.extensions.Permission
import io.eqoty.secretk.types.extensions.Permit
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okio.Path
import okio.Path.Companion.toPath
import kotlin.math.ceil
import kotlin.random.Random
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals

class IntegrationTests {

    private val contractCodePath: Path = getEnv(Constants.CONTRACT_PATH_ENV_NAME)!!.toPath()
    private val purchasePrices = listOf(Coin(amount = 2000000, denom = "uscrt"))

    // Initialization procedure
    private suspend fun initializeAndUploadContract(migrateFrom: MigrateFrom? = null): ContractInfo {
        val initMsg = if (migrateFrom == null) {
            PurchasableSnip721Msgs.Instantiate(
                prices = purchasePrices,
                publicMetadata = Snip721Msgs.Metadata("publicMetadataUri"),
                privateMetadata = Snip721Msgs.Metadata("privateMetadataUri"),
                admin = client.senderAddress,
                entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
            )
        } else {
            PurchasableSnip721Msgs.Instantiate(
                migrateFrom = migrateFrom,
                entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
            )
        }
        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = client.senderAddress,
                codeId = null, // will be set later
                initMsg = Json.encodeToString(initMsg),
                label = "My Snip721" + ceil(Random.nextDouble() * 10000),
                codeHash = null // will be set later
            )
        )
        return DeployContractUtils.storeCodeAndInstantiate(
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

    suspend fun getNumTokensOfOwner(
        ownerAddress: String,
        permit: Permit,
        contractAddr: String
    ): Snip721Msgs.QueryAnswer.NumTokens {
        val numTokensQuery = Json.encodeToString(
            Snip721Msgs.Query(
                withPermit = Snip721Msgs.Query.WithPermit(
                    permit,
                    query = Snip721Msgs.QueryWithPermit(
                        numTokensOfOwner = Snip721Msgs.QueryWithPermit.NumTokensOfOwner(ownerAddress),
                    )
                )
            )
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(
            client.queryContractSmart(
                contractAddr,
                numTokensQuery
            )
        ).numTokens!!
    }

    suspend fun getContractInfo(contractInfo: ContractInfo): Snip721Msgs.QueryAnswer.ContractInfo {
        val query = Snip721Msgs.Query(contractInfo = Snip721Msgs.Query.ContractInfo())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).contractInfo!!
    }

    suspend fun getContractConfig(contractInfo: ContractInfo): Snip721Msgs.QueryAnswer.ContractConfig {
        val query = Snip721Msgs.Query(contractConfig = Snip721Msgs.Query.ContractConfig())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).contractConfig!!
    }

    suspend fun getPurchasePrice(contractInfo: ContractInfo): List<Coin> {
        val query = PurchasableSnip721Msgs.Query(getPrices = PurchasableSnip721Msgs.Query.GetPrices())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeInfo.codeHash
        )
        return Json.decodeFromString<EqotyPurchaseMsgs.QueryAnswer>(res).getPrices!!.prices
    }

    @BeforeTest
    fun beforeEach() = runTest {
        Logger.setTag("dapp")
        if (!clientInitialized) {
            val endpoint = testnetInfo.grpcGatewayEndpoint
            initializeClient(endpoint, testnetInfo.chainId)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000)
        }
    }

    @Test
    fun test_purchase_one_and_migrate() = runTest {
        val contractInfoV1 = initializeAndUploadContract()
        Logger.i("v1 contractInfo: $contractInfoV1")
        val permit = PermitFactory.newPermit(
            client.wallet,
            client.senderAddress,
            client.getChainId(),
            "test",
            listOf(contractInfoV1.address),
            listOf(Permission.Owner)
        )
        val startingNumTokensOfOwner = getNumTokensOfOwner(
            client.senderAddress,
            permit,
            contractInfoV1.address
        ).count
        val purchaseOneMintResult =
            purchaseOneMint(client, contractInfoV1, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner = getNumTokensOfOwner(
            client.senderAddress,
            permit,
            contractInfoV1.address
        ).count
        assertEquals(startingNumTokensOfOwner + 1, numTokensOfOwner)
        val migrateFrom = MigrateFrom(
            contractInfoV1.address,
            contractInfoV1.codeInfo.codeHash,
            permit
        )
        val contractInfoV2 = initializeAndUploadContract(migrateFrom)
        Logger.i("v2 contractInfoMigrated: $contractInfoV2")

        assertEquals(getContractInfo(contractInfoV1), getContractInfo(contractInfoV2))
        assertEquals(getContractConfig(contractInfoV1), getContractConfig(contractInfoV2))
        assertEquals(getPurchasePrice(contractInfoV1), getPurchasePrice(contractInfoV2))

    }


}
