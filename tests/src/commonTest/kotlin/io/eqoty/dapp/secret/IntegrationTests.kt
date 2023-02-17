package io.eqoty.dapp.secret

import DeployContractUtils
import co.touchlab.kermit.Logger
import io.eqoty.dapp.secret.TestGlobals.client
import io.eqoty.dapp.secret.TestGlobals.clientInitialized
import io.eqoty.dapp.secret.TestGlobals.initializeClient
import io.eqoty.dapp.secret.TestGlobals.intializeAccountBeforeExecuteWorkaround
import io.eqoty.dapp.secret.TestGlobals.testnetInfo
import io.eqoty.dapp.secret.types.ContractInfo
import io.eqoty.dapp.secret.types.ExecuteResult
import io.eqoty.dapp.secret.types.MintedRelease
import io.eqoty.dapp.secret.types.contract.*
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
import kotlin.test.*

class IntegrationTests {

    private val snip721DealerContractCodePath: Path =
        "${getEnv(Constants.CONTRACT_PATH_ENV_NAME)}/snip721_dealer.wasm.gz".toPath()
    private val snip721MigratableContractCodePath: Path =
        "${getEnv(Constants.CONTRACT_PATH_ENV_NAME)}/snip721_migratable.wasm.gz".toPath()
    private val purchasePrices = listOf(Coin(amount = 2000000, denom = "uscrt"))

    // Initialization procedure
    private suspend fun initializeAndUploadDealerContract(migrateFrom: MigrationMsg.MigrateFrom? = null): ContractInfo {
        val snip721MigratableCodeInfo = DeployContractUtils.getOrStoreCode(client, snip721MigratableContractCodePath)
        val initMsg = if (migrateFrom == null) {
            Snip721DealerMsgs.Instantiate(
                new = Snip721DealerMsgs.Instantiate.InstantiateSelfAnChildSnip721Msg(
                    snip721CodeInfo = CosmWasmStd.CodeInfo(
                        snip721MigratableCodeInfo.codeId.toULong(),
                        snip721MigratableCodeInfo.codeHash
                    ),
                    snip721Label = "MigratableSnip721" + ceil(Random.nextDouble() * 10000),
                    prices = purchasePrices,
                    publicMetadata = Snip721Msgs.Metadata("publicMetadataUri"),
                    privateMetadata = Snip721Msgs.Metadata("privateMetadataUri"),
                    admin = client.senderAddress,
                    entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
                )
            )
        } else {
            Snip721DealerMsgs.Instantiate(
                migrate = MigrationMsg.InstantiateByMigration(
                    migrateFrom = migrateFrom,
                    entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
                )
            )
        }
        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = client.senderAddress,
                codeId = null, // will be set later
                initMsg = Json.encodeToString(initMsg),
                label = "Snip721Dealer" + ceil(Random.nextDouble() * 10000),
                codeHash = null // will be set later
            )
        )
        return DeployContractUtils.getOrStoreCodeAndInstantiate(
            client,
            snip721DealerContractCodePath,
            instantiateMsgs,
        )
    }

    private suspend fun migrateSnip721Contract(migrateFrom: MigrationMsg.MigrateFrom): ContractInfo {
        val snip721MigratableCodeInfo = DeployContractUtils.getOrStoreCode(client, snip721MigratableContractCodePath)
        val instantiateByMigration = Snip721MigratableMsg.Instantiate(
            migrate = MigrationMsg.InstantiateByMigration(
                migrateFrom = migrateFrom,
                entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
            )
        )

        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = client.senderAddress,
                codeId = null, // will be set later
                initMsg = Json.encodeToString(instantiateByMigration),
                label = "Snip721Migratable" + ceil(Random.nextDouble() * 10000),
                codeHash = null // will be set later
            )
        )
        return DeployContractUtils.instantiateCode(
            client,
            snip721MigratableCodeInfo,
            instantiateMsgs,
        )
    }

    private suspend fun purchaseOneMint(
        client: SigningCosmWasmClient,
        contractInfo: CosmWasmStd.ContractInfo,
        sentFunds: List<Coin>
    ): ExecuteResult<MintedRelease> {
        val purchaseMintMsg = Json.encodeToString(
            Snip721DealerMsgs.Execute(
                purchaseMint = Snip721DealerMsgs.Execute.PurchaseMint()
            )
        )
        val msgs = listOf(
            MsgExecuteContract(
                sender = client.senderAddress,
                contractAddress = contractInfo.address,
                codeHash = contractInfo.codeHash,
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

    private suspend fun migrateTokens(
        client: SigningCosmWasmClient,
        contractInfo: CosmWasmStd.ContractInfo
    ): ExecuteResult<Snip721DealerMsgs.ExecuteAnswer.MigrateTokensIn> {
        val msg = Json.encodeToString(
            Snip721DealerMsgs.Execute(
                migrateTokensIn = Snip721DealerMsgs.Execute.MigrateTokensIn()
            )
        )
        val msgs = listOf(
            MsgExecuteContract(
                sender = client.senderAddress,
                contractAddress = contractInfo.address,
                codeHash = contractInfo.codeHash,
                msg = msg,
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

    suspend fun getContractInfo(contractInfo: CosmWasmStd.ContractInfo): Snip721Msgs.QueryAnswer.ContractInfo {
        val query = Snip721Msgs.Query(contractInfo = Snip721Msgs.Query.ContractInfo())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).contractInfo!!
    }

    suspend fun getContractConfig(contractInfo: CosmWasmStd.ContractInfo): Snip721Msgs.QueryAnswer.ContractConfig {
        val query = Snip721Msgs.Query(contractConfig = Snip721Msgs.Query.ContractConfig())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).contractConfig!!
    }

    suspend fun getPurchasePrice(contractInfo: CosmWasmStd.ContractInfo): List<Coin> {
        val query = Snip721DealerMsgs.Query(getPrices = Snip721DealerMsgs.Query.GetPrices())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721DealerMsgs.QueryAnswer>(res).getPrices!!.prices
    }

    suspend fun getMigratedToContractInfo(contractInfo: CosmWasmStd.ContractInfo): CosmWasmStd.ContractInfo? {
        val query = Snip721DealerMsgs.Query(migratedTo = Snip721DealerMsgs.Query.MigratedTo())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721DealerMsgs.QueryAnswer>(res).migrationInfo
    }

    suspend fun getMigratedFromContractInfo(contractInfo: CosmWasmStd.ContractInfo): CosmWasmStd.ContractInfo? {
        val query = Snip721DealerMsgs.Query(migratedFrom = Snip721DealerMsgs.Query.MigratedFrom())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721DealerMsgs.QueryAnswer>(res).migrationInfo
    }

    suspend fun getChildSnip721ContractInfo(contractInfo: CosmWasmStd.ContractInfo): CosmWasmStd.ContractInfo {
        val query = Snip721DealerMsgs.Query(getChildSnip721 = Snip721DealerMsgs.Query.GetChildSnip721())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721DealerMsgs.QueryAnswer>(res).contractInfo!!
    }

    suspend fun getNumTokens(contractInfo: CosmWasmStd.ContractInfo): Int {
        val query = Snip721Msgs.Query(numTokens = Snip721Msgs.Query.NumTokens())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).numTokens!!.count
    }

    suspend fun getBatchNftDossiers(
        contractInfo: CosmWasmStd.ContractInfo,
        permit: Permit,
        tokenIds: List<String>
    ): Snip721Msgs.QueryAnswer.BatchNftDossier {
        val query = Snip721Msgs.Query(
            withPermit = Snip721Msgs.Query.WithPermit(
                permit = permit,
                query = Snip721Msgs.QueryWithPermit(
                    batchNftDossier = Snip721Msgs.QueryWithPermit.BatchNftDossier(tokenIds)
                )
            )
        )
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        val json = Json { ignoreUnknownKeys = true }
        // workaround deserialize public_ownership_expiration by ignoring it.
        return json.decodeFromString<Snip721Msgs.QueryAnswer>(res).batchNftDossier!!
    }

    suspend fun getTxHistory(
        contractInfo: ContractInfo,
        permit: Permit,
    ): Snip721Msgs.QueryAnswer.TransactionHistory {
        val query = Snip721Msgs.Query(
            withPermit = Snip721Msgs.Query.WithPermit(
                permit = permit,
                query = Snip721Msgs.QueryWithPermit(
                    transactionHistory = Snip721Msgs.QueryWithPermit.TransactionHistory()
                )
            )
        )
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeInfo.codeHash
        )
        val json = Json { ignoreUnknownKeys = true }
        // workaround deserialize public_ownership_expiration by ignoring it.
        return json.decodeFromString<Snip721Msgs.QueryAnswer>(res).transactionHistory!!
    }

    @BeforeTest
    fun beforeEach() = runTest {
        Logger.setTag("dapp")
        if (!clientInitialized) {
            val endpoint = testnetInfo.grpcGatewayEndpoint
            initializeClient(endpoint, testnetInfo.chainId, 3)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000, client.wallet.getAccounts()[0].address)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000, client.wallet.getAccounts()[1].address)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000, client.wallet.getAccounts()[2].address)
            val workaroundContract = initializeAndUploadDealerContract()
            intializeAccountBeforeExecuteWorkaround(workaroundContract, client.wallet.getAccounts()[0].address)
            intializeAccountBeforeExecuteWorkaround(workaroundContract, client.wallet.getAccounts()[1].address)
            intializeAccountBeforeExecuteWorkaround(workaroundContract, client.wallet.getAccounts()[2].address)
        }
        client.senderAddress = client.wallet.getAccounts()[0].address
    }

    @Test
    fun test_purchase_one_and_migrate() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        val permitsV1 = client.wallet.getAccounts().map { account ->
            PermitFactory.newPermit(
                client.wallet,
                account.address,
                client.getChainId(),
                "test",
                listOf(snip721ContractV1.address),
                listOf(Permission.Owner)
            )
        }
        val startingNumTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, permitsV1[1], snip721ContractV1.address).count
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, permitsV1[1], snip721ContractV1.address).count
        assertEquals(startingNumTokensOfOwner + 1, numTokensOfOwner)

        val snip721ContractInfoQueryV1 = getContractInfo(snip721ContractV1)
        val contractConfigV1 = getContractConfig(snip721ContractV1)
        val numTokensV1 = getNumTokens(snip721ContractV1)
        val nftDossiersV1 = getBatchNftDossiers(snip721ContractV1, permitsV1[1], listOf("0"))

        client.senderAddress = client.wallet.getAccounts()[0].address
        val migrateFrom = MigrationMsg.MigrateFrom(
            snip721ContractV1.address,
            snip721ContractV1.codeHash,
            permitsV1[0]
        )
        val snip721ContractInfoV2 = with(migrateSnip721Contract(migrateFrom)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        migrateTokens(client, snip721ContractInfoV2)

        client.senderAddress = client.wallet.getAccounts()[1].address
        val permit = PermitFactory.newPermit(
            client.wallet,
            client.senderAddress,
            client.getChainId(),
            "test",
            listOf(snip721ContractInfoV2.address),
            listOf(Permission.Owner)
        )

        assertNotEquals(snip721ContractV1.address, snip721ContractInfoV2.address)
        assertEquals(
            snip721ContractInfoQueryV1,
            getContractInfo(snip721ContractInfoV2)
        )
        assertEquals(contractConfigV1, getContractConfig(snip721ContractInfoV2))
        assertEquals(numTokensV1, getNumTokens(snip721ContractInfoV2))
        val json = Json { prettyPrint = true }
        val nftDossiersV2 = getBatchNftDossiers(snip721ContractInfoV2, permit, listOf("0"))
        assertTrue(
            nftDossiersV1.equals(
                nftDossiersV2,
                ignoreCollectionCreator = true,
                ignoreTokenCreator = true,
                ignoreTimeOfMinting = true
            ),
            "expected:\n${json.encodeToString(nftDossiersV1)}\nactual:\n${json.encodeToString(nftDossiersV2)}"
        )
    }

    @Test
    fun test_snip721_migrated_info() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val dealerQueriedSnip721V1 = getChildSnip721ContractInfo(dealerContractInfo)
        val migratedFromInfoV1 = getMigratedFromContractInfo(dealerQueriedSnip721V1)
        assertEquals(null, migratedFromInfoV1)
        var migratedToInfoV1 = getMigratedToContractInfo(dealerContractInfo)
        assertEquals(null, migratedToInfoV1)
        val permitsV1 = client.wallet.getAccounts().map { account ->
            PermitFactory.newPermit(
                client.wallet,
                account.address,
                client.getChainId(),
                "test",
                listOf(dealerQueriedSnip721V1.address),
                listOf(Permission.Owner)
            )
        }
        val migrateFrom = MigrationMsg.MigrateFrom(
            dealerQueriedSnip721V1.address,
            dealerQueriedSnip721V1.codeHash,
            permitsV1[0]
        )
        val snip721ContractInfoV2 = with(migrateSnip721Contract(migrateFrom)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        assertNotEquals(dealerQueriedSnip721V1, snip721ContractInfoV2)

        migratedToInfoV1 = getMigratedToContractInfo(dealerQueriedSnip721V1)
        assertEquals(snip721ContractInfoV2, migratedToInfoV1)

        var migratedFromInfoV2 = getMigratedFromContractInfo(snip721ContractInfoV2)
        assertEquals(dealerQueriedSnip721V1, migratedFromInfoV2)

        var migratedToInfoV2 = getMigratedToContractInfo(snip721ContractInfoV2)

        assertEquals(null, migratedToInfoV2)

        migrateTokens(client, snip721ContractInfoV2)

        // test again to make sure queries are still available after contract changes mode to Running
        migratedToInfoV1 = getMigratedToContractInfo(dealerQueriedSnip721V1)
        assertEquals(snip721ContractInfoV2, migratedToInfoV1)
        
        migratedFromInfoV2 = getMigratedFromContractInfo(snip721ContractInfoV2)
        assertEquals(dealerQueriedSnip721V1, migratedFromInfoV2)

        migratedToInfoV2 = getMigratedToContractInfo(snip721ContractInfoV2)
        assertEquals(null, migratedToInfoV2)
    }


}
