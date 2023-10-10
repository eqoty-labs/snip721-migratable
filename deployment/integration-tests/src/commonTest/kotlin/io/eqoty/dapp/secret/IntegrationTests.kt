package io.eqoty.dapp.secret

import DeployContractUtils
import co.touchlab.kermit.Logger
import io.eqoty.cosmwasm.std.types.CodeInfo
import io.eqoty.cosmwasm.std.types.Coin
import io.eqoty.cosmwasm.std.types.ContractInfo
import io.eqoty.dapp.secret.TestGlobals.client
import io.eqoty.dapp.secret.TestGlobals.clientInitialized
import io.eqoty.dapp.secret.TestGlobals.initializeClient
import io.eqoty.dapp.secret.TestGlobals.intializeAccountBeforeExecuteWorkaround
import io.eqoty.dapp.secret.TestGlobals.testnetInfo
import io.eqoty.dapp.secret.types.ContractInstance
import io.eqoty.dapp.secret.types.ExecuteResult
import io.eqoty.dapp.secret.types.contract.Snip721DealerMsgs
import io.eqoty.dapp.secret.types.contract.equals
import io.eqoty.dapp.secret.utils.BalanceUtils
import io.eqoty.dapp.secret.utils.Constants
import io.eqoty.secret.std.contract.msg.Snip721Msgs
import io.eqoty.secret.std.types.Permission
import io.eqoty.secret.std.types.Permit
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.extensions.accesscontrol.PermitFactory
import io.eqoty.secretk.types.MsgExecuteContract
import io.eqoty.secretk.types.MsgInstantiateContract
import io.eqoty.secretk.types.MsgMigrateContract
import io.eqoty.secretk.types.TxOptions
import io.getenv
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okio.Path
import okio.Path.Companion.toPath
import kotlin.math.ceil
import kotlin.random.Random
import kotlin.test.*
import kotlin.time.Duration.Companion.seconds

class IntegrationTests {

    private val snip721DealerContractCodePath: Path =
        "${getenv(Constants.CONTRACT_PATH_ENV_NAME)}/snip721_dealer.wasm.gz".toPath()
    private val snip721DealerContractOptimizedCodePath: Path =
        "${getenv(Constants.CONTRACT_PATH_ENV_NAME)}/snip721_dealer-optimized.wasm.gz".toPath()
    private val snip721MigratableContractCodePath: Path =
        "${getenv(Constants.CONTRACT_PATH_ENV_NAME)}/snip721_migratable.wasm.gz".toPath()
    private val snip721MigratableContractOptimizedCodePath: Path =
        "${getenv(Constants.CONTRACT_PATH_ENV_NAME)}/snip721_migratable-optimized.wasm.gz".toPath()
    private val purchasePrices = listOf(Coin(amount = 2000000, denom = "uscrt"))
    private suspend fun snip721MigratableCodeInfo(senderAddress: String): CodeInfo {
        return DeployContractUtils.getOrStoreCode(client, senderAddress, snip721MigratableContractCodePath, null)
    }

    private suspend fun snip721MigratableOptimizedCodeInfo(senderAddress: String): CodeInfo {
        return DeployContractUtils.getOrStoreCode(
            client, senderAddress, snip721MigratableContractOptimizedCodePath, null
        )
    }

    private suspend fun snip721DealerCodeInfo(senderAddress: String): CodeInfo {
        return DeployContractUtils.getOrStoreCode(client, senderAddress, snip721DealerContractCodePath, null)
    }

    private suspend fun snip721DealerOptimizedCodeInfo(senderAddress: String): CodeInfo {
        return DeployContractUtils.getOrStoreCode(client, senderAddress, snip721DealerContractOptimizedCodePath, null)
    }

    // Initialization procedure
    private suspend fun instantiateUnoptimizedDealerContract(senderAddress: String): ContractInfo {
        val initMsg = Snip721DealerMsgs.Instantiate(
            snip721CodeId = snip721MigratableCodeInfo(senderAddress).codeId.toULong(),
            snip721CodeHash = snip721MigratableCodeInfo(senderAddress).codeHash,
            snip721Label = "MigratableSnip721" + ceil(Random.nextDouble() * 1000000),
            prices = purchasePrices,
            publicMetadata = Snip721Msgs.Metadata("publicMetadataUri"),
            privateMetadata = Snip721Msgs.Metadata("privateMetadataUri"),
            admin = senderAddress,
            entropy = "sometimes you gotta close a door to open a window: " + Random.nextDouble().toString()

        )
        val instantiateMsgs = listOf(
            MsgInstantiateContract(
                sender = senderAddress,
                codeId = -1, // will be set later
                initMsg = Json.encodeToString(initMsg),
                label = "Snip721Dealer" + ceil(Random.nextDouble() * 1000000),
                codeHash = null, // will be set later
                admin = senderAddress
            )
        )
        return DeployContractUtils.instantiateCode(client, snip721DealerCodeInfo(senderAddress), instantiateMsgs, 500_000)
            .let {
                ContractInfo(
                    it.address, it.codeInfo.codeHash
                )
            }
    }

    private suspend fun migrateToOptimizedSnip721Contract(senderAddress: String, contract: ContractInfo) =
        migrateContract(senderAddress, contract, snip721MigratableOptimizedCodeInfo(senderAddress))

    private suspend fun migrateToOptimizedSnip721Dealer(
        senderAddress: String, contract: ContractInfo
    ) = migrateContract(senderAddress, contract, snip721DealerOptimizedCodeInfo(senderAddress))

    private suspend fun migrateContract(
        senderAddress: String, contract: ContractInfo, codeInfo: CodeInfo
    ): ContractInfo {
        val migrateMsgs = listOf(
            MsgMigrateContract(
                sender = senderAddress,
                contractAddress = contract.address,
                codeId = codeInfo.codeId.toInt(),
                msg = "{}",
                codeHash = codeInfo.codeHash
            )
        )
        val res = client.execute(migrateMsgs, TxOptions(gasLimit = 500_000))
        val migrateAttributes = res.logs[0].events.find { it.type == "migrate" }?.attributes
        val contractAddress = migrateAttributes?.find { it.key == "contract_address" }?.value!!
//        val migratedToCodeId = migrateAttributes.find { it.key == "code_id" }?.value!!.toInt()
//        assertNotEquals(codeIdBeforeMigrate, migratedToCodeId)
//        println("codeIdBeforeMigrate:$codeIdBeforeMigrate vs migratedToCodeId:$migratedToCodeId")
        val contractCodeHashAfterMigrate = client.getCodeHashByContractAddr(contractAddress)
        return ContractInfo(
            contractAddress, contractCodeHashAfterMigrate
        )
    }

    private suspend fun purchaseOneMint(
        client: SigningCosmWasmClient, senderAddress: String, contractInfo: ContractInfo, sentFunds: List<Coin>
    ): ExecuteResult<Any> {
        val purchaseMintMsg = Json.encodeToString(
            Snip721DealerMsgs.Execute(
                purchaseMint = Snip721DealerMsgs.Execute.PurchaseMint()
            )
        )
        val msgs = listOf(
            MsgExecuteContract(
                sender = senderAddress,
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
                msgs, txOptions = txOptions
            )
        } catch (t: Throwable) {
            Logger.i(t.message ?: "")
            null
        }
        val gasFee = client.gasToFee(txOptions.gasLimit, txOptions.gasPriceInFeeDenom)
        return ExecuteResult(res, Coin(gasFee, "uscrt"))
    }

    suspend fun getNumTokensOfOwner(
        ownerAddress: String, contractAddr: String
    ): Snip721Msgs.QueryAnswer.NumTokens {
        val permit = PermitFactory.newPermit(
            client.wallet!!, ownerAddress, client.getChainId(), "test", listOf(contractAddr), listOf(Permission.Owner)
        )
        val numTokensQuery = Json.encodeToString(
            Snip721Msgs.Query(
                withPermit = Snip721Msgs.Query.WithPermit(
                    permit, query = Snip721Msgs.QueryWithPermit(
                        numTokensOfOwner = Snip721Msgs.QueryWithPermit.NumTokensOfOwner(ownerAddress),
                    )
                )
            )
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(
            client.queryContractSmart(
                contractAddr, numTokensQuery
            )
        ).numTokens!!
    }

    suspend fun getSnip721ContractInfo(contractInfo: ContractInfo): Snip721Msgs.QueryAnswer.ContractInfo {
        val query = Snip721Msgs.Query(contractInfo = Snip721Msgs.Query.ContractInfo())
        val res = client.queryContractSmart(
            contractInfo.address, Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).contractInfo!!
    }

    suspend fun getContractConfig(contractInfo: ContractInfo): Snip721Msgs.QueryAnswer.ContractConfig {
        val query = Snip721Msgs.Query(contractConfig = Snip721Msgs.Query.ContractConfig())
        val res = client.queryContractSmart(
            contractInfo.address, Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).contractConfig!!
    }


    suspend fun getChildSnip721ContractInfo(contractInfo: ContractInfo): ContractInfo {
        val query = Snip721DealerMsgs.Query(getChildSnip721 = Snip721DealerMsgs.Query.GetChildSnip721())
        val res = client.queryContractSmart(
            contractInfo.address, Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721DealerMsgs.QueryAnswer>(res).contractInfo!!
    }

    suspend fun getMinters(contractInfo: ContractInfo): Snip721Msgs.QueryAnswer.Minters {
        val query = Snip721Msgs.Query(minters = Snip721Msgs.Query.Minters())
        val res = client.queryContractSmart(
            contractInfo.address, Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).minters!!
    }

    suspend fun getNumTokens(contractInfo: ContractInfo): Int {
        val query = Snip721Msgs.Query(numTokens = Snip721Msgs.Query.NumTokens())
        val res = client.queryContractSmart(
            contractInfo.address, Json.encodeToString(query), contractInfo.codeHash
        )
        return Json.decodeFromString<Snip721Msgs.QueryAnswer>(res).numTokens!!.count
    }

    suspend fun getBatchNftDossiers(
        senderAddress: String, contractInfo: ContractInfo, tokenIds: List<String>
    ): Snip721Msgs.QueryAnswer.BatchNftDossier {
        val permit = PermitFactory.newPermit(
            client.wallet!!,
            senderAddress,
            client.getChainId(),
            "test",
            listOf(contractInfo.address),
            listOf(Permission.Owner)
        )
        val query = Snip721Msgs.Query(
            withPermit = Snip721Msgs.Query.WithPermit(
                permit = permit, query = Snip721Msgs.QueryWithPermit(
                    batchNftDossier = Snip721Msgs.QueryWithPermit.BatchNftDossier(tokenIds)
                )
            )
        )
        val res = client.queryContractSmart(
            contractInfo.address, Json.encodeToString(query), contractInfo.codeHash
        )
        val json = Json { ignoreUnknownKeys = true }
        // workaround deserialize public_ownership_expiration by ignoring it.
        return json.decodeFromString<Snip721Msgs.QueryAnswer>(res).batchNftDossier!!
    }

    @BeforeTest
    fun beforeEach() = runTest(timeout = 60.seconds) {
        Logger.setTag("tests")
        if (!clientInitialized) {
            val endpoint = testnetInfo.grpcGatewayEndpoint
            initializeClient(endpoint, testnetInfo.chainId, 3)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000, client.wallet!!.getAccounts()[0].address)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000, client.wallet!!.getAccounts()[1].address)
            BalanceUtils.fillUpFromFaucet(testnetInfo, client, 100_000_000, client.wallet!!.getAccounts()[2].address)
            intializeAccountBeforeExecuteWorkaround(client.wallet!!.getAccounts()[0].address)
            intializeAccountBeforeExecuteWorkaround(client.wallet!!.getAccounts()[1].address)
            intializeAccountBeforeExecuteWorkaround(client.wallet!!.getAccounts()[2].address)
        }
    }

    @Test
    fun dealer_can_mint_after_dealer_migrates() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val dealerContractV1 = instantiateUnoptimizedDealerContract(senderAddress0)
        val dealerContractV2 = migrateToOptimizedSnip721Dealer(senderAddress0, dealerContractV1)
        assertNotEquals(dealerContractV1, dealerContractV2)
        val snip721Contract = getChildSnip721ContractInfo(dealerContractV2)
        val senderAddress1 = client.wallet!!.getAccounts()[1].address
        purchaseOneMint(client, senderAddress1, dealerContractV2, purchasePrices)

        assertEquals(
            1, getNumTokensOfOwner(senderAddress1, snip721Contract.address).count
        )
    }

    @Test
    fun purchase_one_and_migrate_snip721() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val senderAddress1 = client.wallet!!.getAccounts()[1].address
        val dealerContractInfo = instantiateUnoptimizedDealerContract(senderAddress0)
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)


        purchaseOneMint(client, senderAddress1, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner = getNumTokensOfOwner(senderAddress1, snip721ContractV1.address).count
        assertEquals(1, numTokensOfOwner)

        val snip721ContractInfoQueryV1 = getSnip721ContractInfo(snip721ContractV1)
        val contractConfigV1 = getContractConfig(snip721ContractV1)
        val numTokensV1 = getNumTokens(snip721ContractV1)
        val nftDossiersV1 = getBatchNftDossiers(senderAddress1, snip721ContractV1, listOf("0"))


        val snip721ContractInfoV2 = migrateToOptimizedSnip721Contract(senderAddress0, snip721ContractV1)
        assertNotEquals(snip721ContractV1, snip721ContractInfoV2)

        assertEquals(
            snip721ContractInfoQueryV1, getSnip721ContractInfo(snip721ContractInfoV2)
        )
        assertEquals(contractConfigV1, getContractConfig(snip721ContractInfoV2))
        assertEquals(numTokensV1, getNumTokens(snip721ContractInfoV2))
        val json = Json { prettyPrint = true }
        val nftDossiersV2 = getBatchNftDossiers(senderAddress1, snip721ContractInfoV2, listOf("0"))
        assertTrue(
            nftDossiersV1.equals(
                nftDossiersV2, ignoreCollectionCreator = true, ignoreTokenCreator = true, ignoreTimeOfMinting = true
            ), "expected:\n${json.encodeToString(nftDossiersV1)}\nactual:\n${json.encodeToString(nftDossiersV2)}"
        )
    }

    @Test
    fun dealer_is_notified_of_migrated_child_snip721_address() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val senderAddress1 = client.wallet!!.getAccounts()[1].address
        val dealerContractInfo = instantiateUnoptimizedDealerContract(senderAddress0)
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        val startingNumTokensOfOwner = getNumTokensOfOwner(senderAddress1, snip721ContractV1.address).count
        purchaseOneMint(client, senderAddress1, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        val numTokensOfOwner = getNumTokensOfOwner(senderAddress1, snip721ContractV1.address).count
        assertEquals(startingNumTokensOfOwner + 1, numTokensOfOwner)

        val snip721ContractInfoV2 = migrateToOptimizedSnip721Contract(senderAddress0, snip721ContractV1)
        assertNotEquals(snip721ContractV1, snip721ContractInfoV2)

        assertEquals(snip721ContractInfoV2, getChildSnip721ContractInfo(dealerContractInfo))
    }

    @Test
    fun minters_are_migrated() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val dealerContractInfo = instantiateUnoptimizedDealerContract(senderAddress0)
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        val mintersV1 = getMinters(snip721ContractV1)
        val snip721ContractInfoV2 = migrateToOptimizedSnip721Contract(senderAddress0, snip721ContractV1)
        assertNotEquals(snip721ContractV1, snip721ContractInfoV2)

        val mintersV2 = getMinters(snip721ContractInfoV2)
        assertEquals(mintersV1, mintersV2)
    }

    @Test
    fun dealer_can_mint_after_snip721_migrates_tokens() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val senderAddress1 = client.wallet!!.getAccounts()[1].address
        val dealerContractInfo = instantiateUnoptimizedDealerContract(senderAddress0)
        val snip721ContractV1 = getChildSnip721ContractInfo(dealerContractInfo)
        purchaseOneMint(client, senderAddress1, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        var numTokensOfOwner = getNumTokensOfOwner(senderAddress1, snip721ContractV1.address).count
        assertEquals(1, numTokensOfOwner)

        val snip721ContractInfoV2 = migrateToOptimizedSnip721Contract(senderAddress0, snip721ContractV1)
        assertNotEquals(snip721ContractV1, snip721ContractInfoV2)

        purchaseOneMint(client, senderAddress1, dealerContractInfo, purchasePrices)
        // verify customer received one nft
        numTokensOfOwner = getNumTokensOfOwner(senderAddress1, snip721ContractInfoV2.address).count
        assertEquals(2, numTokensOfOwner)
    }

    @Test
    fun non_admin_cannot_migrate_dealer() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val senderAddress1 = client.wallet!!.getAccounts()[1].address
        val dealerContract = instantiateUnoptimizedDealerContract(senderAddress0)
        val errorMessage = try {
            migrateToOptimizedSnip721Dealer(senderAddress1, dealerContract)
            ""
        } catch (t: Throwable) {
            t.message!!
        }
        assertContains(errorMessage, "requires migrate from admin: migrate contract failed")
    }

    @Test
    fun non_admin_permit_cannot_migrate_snip721() = runTest(timeout = 60.seconds) {
        val senderAddress0 = client.wallet!!.getAccounts()[0].address
        val senderAddress1 = client.wallet!!.getAccounts()[1].address
        val dealerContract = instantiateUnoptimizedDealerContract(senderAddress0)
        val snip721Contract = getChildSnip721ContractInfo(dealerContract)
        val errorMessage = try {
            migrateToOptimizedSnip721Dealer(senderAddress1, snip721Contract)
            ""
        } catch (t: Throwable) {
            t.message!!
        }
        assertContains(errorMessage, "requires migrate from admin: migrate contract failed")
    }

}
