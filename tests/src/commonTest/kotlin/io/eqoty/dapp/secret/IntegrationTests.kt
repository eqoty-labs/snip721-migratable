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
import io.eqoty.dapp.secret.types.contract.CosmWasmStd
import io.eqoty.dapp.secret.types.contract.Snip721DealerMsgs
import io.eqoty.dapp.secret.types.contract.Snip721Msgs
import io.eqoty.dapp.secret.types.contract.equals
import io.eqoty.dapp.secret.types.contract.migratable.MigratableContractMsg
import io.eqoty.dapp.secret.types.contract.migratable.MigratableContractTypes
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
    private suspend fun initializeAndUploadDealerContract(migrateFrom: MigratableContractTypes.MigrateFrom? = null): ContractInfo {
        val snip721MigratableCodeInfo = DeployContractUtils.getOrStoreCode(client, snip721MigratableContractCodePath)
        val initMsg = if (migrateFrom == null) {
            Snip721DealerMsgs.Instantiate(
                new = Snip721DealerMsgs.Instantiate.InstantiateSelfAnChildSnip721Msg(
                    snip721CodeId = snip721MigratableCodeInfo.codeId.toULong(),
                    snip721CodeHash = snip721MigratableCodeInfo.codeHash,
                    snip721Label = "MigratableSnip721" + ceil(Random.nextDouble() * 1000000),
                    prices = purchasePrices,
                    publicMetadata = Snip721Msgs.Metadata("publicMetadataUri"),
                    privateMetadata = Snip721Msgs.Metadata("privateMetadataUri"),
                    admin = client.senderAddress,
                    entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
                )
            )
        } else {
            Snip721DealerMsgs.Instantiate(
                migrate = MigratableContractTypes.InstantiateByMigration(
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
                label = "Snip721Dealer" + ceil(Random.nextDouble() * 1000000),
                codeHash = null // will be set later
            )
        )
        return DeployContractUtils.getOrStoreCodeAndInstantiate(
            client,
            snip721DealerContractCodePath,
            instantiateMsgs,
        )
    }

    private suspend fun migrateSnip721Contract(contract: CosmWasmStd.ContractInfo) =
        migrateContract(contract, "Snip721Migratable", snip721MigratableContractCodePath)

    private suspend fun migrateSnip721Dealer(contract: CosmWasmStd.ContractInfo) =
        migrateContract(contract, "Snip721Dealer", snip721DealerContractCodePath)

    private suspend fun migrateContract(
        contract: CosmWasmStd.ContractInfo,
        labelBase: String,
        codepath: Path
    ): ContractInfo {
        val permit = PermitFactory.newPermit(
            client.wallet,
            client.senderAddress,
            client.getChainId(),
            "test",
            listOf(contract.address),
            listOf(Permission.Owner)
        )
        val migrateFrom = MigratableContractTypes.MigrateFrom(
            contract.address,
            contract.codeHash,
            permit
        )
        val snip721MigratableCodeInfo = DeployContractUtils.getOrStoreCode(client, codepath)
        val instantiateByMigration = MigratableContractMsg.Instantiate(
            migrate = MigratableContractTypes.InstantiateByMigration(
                migrateFrom = migrateFrom,
                entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()
            )
        )

        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = client.senderAddress,
                codeId = null, // will be set later
                initMsg = Json.encodeToString(instantiateByMigration),
                label = labelBase + ceil(Random.nextDouble() * 1000000),
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
        val simulate = client.simulate(msgs)
        val gasLimit = (simulate.gasUsed.toDouble() * 1.1).toInt()

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

    private suspend fun transferNft(
        from: String,
        to: String,
        tokenId: String,
        client: SigningCosmWasmClient,
        contractInfo: CosmWasmStd.ContractInfo,

        ): ExecuteResult<MintedRelease> {
        val savedSender = client.senderAddress
        client.senderAddress = from
        val msg = Json.encodeToString(
            Snip721Msgs.Execute(
                transferNft = Snip721Msgs.Execute.TransferNft(
                    recipient = to,
                    tokenId = tokenId
                )
            )
        )
        val msgs = listOf(
            MsgExecuteContract(
                sender = from,
                contractAddress = contractInfo.address,
                codeHash = contractInfo.codeHash,
                msg = msg,
                sentFunds = listOf()
            )
        )
        val simulate = client.simulate(msgs)
        val gasLimit = (simulate.gasUsed.toDouble() * 1.1).toInt()

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
        client.senderAddress = savedSender
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

        val simulate = client.simulate(msgs)
        val gasLimit = (simulate.gasUsed.toDouble() * 1.1).toInt()

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
        contractAddr: String
    ): Snip721Msgs.QueryAnswer.NumTokens {
        val permit = PermitFactory.newPermit(
            client.wallet,
            client.senderAddress,
            client.getChainId(),
            "test",
            listOf(contractAddr),
            listOf(Permission.Owner)
        )
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

    suspend fun getMinters(contractInfo: CosmWasmStd.ContractInfo): Snip721Msgs.QueryAnswer.Minters {
        val query = Snip721Msgs.Query(minters = Snip721Msgs.Query.Minters())
        val res = client.queryContractSmart(
            contractInfo.address,
            Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).minters!!
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
        tokenIds: List<String>
    ): Snip721Msgs.QueryAnswer.BatchNftDossier {
        val permit = PermitFactory.newPermit(
            client.wallet,
            client.senderAddress,
            client.getChainId(),
            "test",
            listOf(contractInfo.address),
            listOf(Permission.Owner)
        )
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
    fun snip721_migrated_info() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val dealerQueriedSnip721V1 = getChildSnip721ContractInfo(dealerContractInfo)
        val migratedFromInfoV1 = getMigratedFromContractInfo(dealerQueriedSnip721V1)
        assertEquals(null, migratedFromInfoV1)
        var migratedToInfoV1 = getMigratedToContractInfo(dealerQueriedSnip721V1)
        assertEquals(null, migratedToInfoV1)
        val snip721ContractInfoV2 = with(migrateSnip721Contract(dealerQueriedSnip721V1)) {
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

    @Test
    fun dealer_migrated_info() = runTest {
        val dealerContractV1 = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val migratedFromInfoV1 = getMigratedFromContractInfo(dealerContractV1)
        assertEquals(null, migratedFromInfoV1)
        var migratedToInfoV1 = getMigratedToContractInfo(dealerContractV1)
        assertEquals(null, migratedToInfoV1)
        val dealerContractV2 = with(migrateSnip721Dealer(dealerContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        assertNotEquals(dealerContractV1, dealerContractV2)

        migratedToInfoV1 = getMigratedToContractInfo(dealerContractV1)
        assertEquals(dealerContractV2, migratedToInfoV1)

        var migratedFromInfoV2 = getMigratedFromContractInfo(dealerContractV2)
        assertEquals(dealerContractV1, migratedFromInfoV2)

        var migratedToInfoV2 = getMigratedToContractInfo(dealerContractV2)

        assertEquals(null, migratedToInfoV2)


        // test again to make sure queries are still available after contract changes mode to Running
        migratedToInfoV1 = getMigratedToContractInfo(dealerContractV1)
        assertEquals(dealerContractV2, migratedToInfoV1)

        migratedFromInfoV2 = getMigratedFromContractInfo(dealerContractV2)
        assertEquals(dealerContractV1, migratedFromInfoV2)

        migratedToInfoV2 = getMigratedToContractInfo(dealerContractV2)
        assertEquals(null, migratedToInfoV2)
    }

    @Test
    fun dealer_can_mint_after_dealer_migrates() = runTest {
        val dealerContractV1 = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val dealerContractV2 = with(migrateSnip721Dealer(dealerContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val snip721Contract = getChildSnip721ContractInfo(dealerContractV2)
        client.senderAddress = client.wallet.getAccounts()[1].address
        purchaseOneMint(client, dealerContractV2, purchasePrices)

        assertEquals(
            1,
            getNumTokensOfOwner(client.senderAddress, snip721Contract.address).count
        )
    }

    @Test
    fun dealer_can_mint_after_dealer_migrates_twice() = runTest {
        val dealerContractV1 = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val dealerContractV2 = with(migrateSnip721Dealer(dealerContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val dealerContractV3 = with(migrateSnip721Dealer(dealerContractV2)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val snip721Contract = getChildSnip721ContractInfo(dealerContractV3)
        client.senderAddress = client.wallet.getAccounts()[1].address
        purchaseOneMint(client, dealerContractV3, purchasePrices)

        assertEquals(
            1,
            getNumTokensOfOwner(client.senderAddress, snip721Contract.address).count
        )
    }

    @Test
    fun purchase_one_and_migrate_snip721() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)


        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        assertEquals(1, numTokensOfOwner)

        val snip721ContractInfoQueryV1 = getContractInfo(snip721ContractV1)
        val contractConfigV1 = getContractConfig(snip721ContractV1)
        val numTokensV1 = getNumTokens(snip721ContractV1)
        val nftDossiersV1 = getBatchNftDossiers(snip721ContractV1, listOf("0"))

        client.senderAddress = client.wallet.getAccounts()[0].address

        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        migrateTokens(client, snip721ContractInfoV2)

        client.senderAddress = client.wallet.getAccounts()[1].address

        assertNotEquals(snip721ContractV1.address, snip721ContractInfoV2.address)
        assertEquals(
            snip721ContractInfoQueryV1,
            getContractInfo(snip721ContractInfoV2)
        )
        assertEquals(contractConfigV1, getContractConfig(snip721ContractInfoV2))
        assertEquals(numTokensV1, getNumTokens(snip721ContractInfoV2))
        val json = Json { prettyPrint = true }
        val nftDossiersV2 = getBatchNftDossiers(snip721ContractInfoV2, listOf("0"))
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
    fun dealer_is_notified_of_migrated_child_snip721_address() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        val startingNumTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        assertEquals(startingNumTokensOfOwner + 1, numTokensOfOwner)

        client.senderAddress = client.wallet.getAccounts()[0].address
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        migrateTokens(client, snip721ContractInfoV2)

        assertEquals(snip721ContractInfoV2, getChildSnip721ContractInfo(dealerContractInfo))
    }

    @Test
    fun minters_are_migrated() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        val mintersV1 = getMinters(snip721ContractV1)
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        migrateTokens(client, snip721ContractInfoV2)
        val mintersV2 = getMinters(snip721ContractInfoV2)
        assertEquals(mintersV1, mintersV2)
    }

    @Test
    fun dealer_can_mint_after_snip721_migrates_tokens() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        var numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        assertEquals(1, numTokensOfOwner)

        client.senderAddress = client.wallet.getAccounts()[0].address
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        migrateTokens(client, snip721ContractInfoV2)

        client.senderAddress = client.wallet.getAccounts()[1].address

        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractInfoV2.address).count
        assertEquals(2, numTokensOfOwner)
    }

    @Test
    fun dealer_can_mint_after_snip721_migrates_tokens_twice() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        var numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        assertEquals(1, numTokensOfOwner)

        client.senderAddress = client.wallet.getAccounts()[0].address
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        migrateTokens(client, snip721ContractInfoV2)
        val snip721ContractInfoV3 = with(migrateSnip721Contract(snip721ContractInfoV2)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        migrateTokens(client, snip721ContractInfoV3)

        client.senderAddress = client.wallet.getAccounts()[1].address

        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractInfoV3.address).count
        assertEquals(2, numTokensOfOwner)
    }

    @Test
    fun dealer_cannot_mint_before_snip721_migrates_tokens() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner =
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        assertEquals(1, numTokensOfOwner)

        client.senderAddress = client.wallet.getAccounts()[0].address
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        client.senderAddress = client.wallet.getAccounts()[1].address

        val purchaseError = try {
            purchaseOneMint(client, dealerContractInfo, purchasePrices)
            ""
        } catch (t: Throwable) {
            t.message!!
        }
        assertContains(purchaseError, "Not available in contact mode: MigrateOutStarted")
    }

    @Test
    fun snip721_contract_state_being_migrated_cannot_be_altered_but_can_be_queried() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft

        assertEquals(
            1,
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        )

        client.senderAddress = client.wallet.getAccounts()[0].address
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        val transferError = try {
            transferNft(
                client.wallet.getAccounts()[1].address,
                client.wallet.getAccounts()[2].address,
                "0",
                client,
                snip721ContractV1
            )
            ""
        } catch (t: Throwable) {
            t.message!!
        }
        assertContains(transferError, "Not available in contact mode: MigrateOutStarted")

        client.senderAddress = client.wallet.getAccounts()[1].address
        assertEquals(
            1,
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        )

    }

    @Test
    fun migrated_snip721_tokens_cannot_be_queried() = runTest {
        val dealerContractInfo = with(initializeAndUploadDealerContract()) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }
        client.senderAddress = client.wallet.getAccounts()[1].address
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        purchaseOneMint(client, dealerContractInfo, purchasePrices)
        // verify customer received one nft

        assertEquals(
            1,
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
        )

        client.senderAddress = client.wallet.getAccounts()[0].address
        val snip721ContractInfoV2 = with(migrateSnip721Contract(snip721ContractV1)) {
            CosmWasmStd.ContractInfo(address, codeInfo.codeHash)
        }

        migrateTokens(client, snip721ContractInfoV2)

        client.senderAddress = client.wallet.getAccounts()[1].address

        val getNumTokensOfOwnerError = try {
            getNumTokensOfOwner(client.senderAddress, snip721ContractV1.address).count
            ""
        } catch (t: Throwable) {
            t.message!!
        }
        assertContains(getNumTokensOfOwnerError, "Not available in contact mode: MigratedOut")

    }


}
