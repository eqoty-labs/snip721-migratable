package io.eqoty.dapp.secret

import DeployContractUtils
import io.eqoty.cosmwasm.std.types.CodeInfo
import io.eqoty.dapp.secret.utils.*
import io.eqoty.secretk.client.SigningCosmWasmClient
import io.eqoty.secretk.wallet.DirectSigningWallet
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy
import okio.Path
import okio.Path.Companion.toPath
import java.lang.System.getenv


val json = Json {
    prettyPrint = true
    namingStrategy = JsonNamingStrategy.SnakeCase
}

enum class ContractType(val contractName: String, val codePath: Path) {
    SNIP721_DEALER(
        "snip721-dealer",
        "${getenv(Constants.CONTRACT_PATH_ENV_NAME)}/../optimized-wasm/snip721_dealer.wasm.gz".toPath()
    ),
    SNIP721_MIGRATABLE(
        "snip721-migratable",
        "${getenv(Constants.CONTRACT_PATH_ENV_NAME)}/../optimized-wasm/snip721_migratable.wasm.gz".toPath()
    ),
}

data class ContractCodeMetadata(
    val version: String,
    val codeInfo: CodeInfo,
)

suspend fun storeUpdatedContracts(
    client: SigningCosmWasmClient, senderAddress: String, nodeInfo: NodeInfo
): Map<ContractType, ContractCodeMetadata> {
    val outOfDateContractsToVersions = mutableMapOf<ContractType, ContractCodeMetadata>()
    val snip721DealerCargoTomlPath = "../../contracts/snip721-dealer/Cargo.toml".toPath()
    val snip721MigratableCargoTomlPath = "../../contracts/snip721-migratable/Cargo.toml".toPath()
    val contractTypeToCargoTomlPaths = mapOf(
        ContractType.SNIP721_DEALER to snip721DealerCargoTomlPath,
        ContractType.SNIP721_MIGRATABLE to snip721MigratableCargoTomlPath,
    )
    contractTypeToCargoTomlPaths.forEach { (contractType, cargoTomlPath) ->
        val cargoTomlVersion = fileSystem.read(cargoTomlPath) {
            var versionLine = readUtf8Line()!!
            while (!versionLine.startsWith("version")) {
                versionLine = readUtf8Line()!!
            }
            versionLine.substringAfter("\"").substringBefore("\"")
        }
        println("$contractType cargoTomlVersion: $cargoTomlVersion")
        val deployedVersionPath =
            "../../deployed/${contractType.contractName}/v$cargoTomlVersion/${nodeInfo.type}.json".toPath()
        val codeInfo: CodeInfo = when (fileSystem.exists(deployedVersionPath)) {
            true -> {
                val codeInfoJson = fileSystem.read(deployedVersionPath) {
                    readUtf8()
                }
                Json.decodeFromString(codeInfoJson)
            }

            false -> {
                println("$contractType out of date, storing version $cargoTomlVersion")
                val codeInfo = DeployContractUtils.storeCode(client, senderAddress, contractType.codePath, null)
                val codeInfoJson = json.encodeToString(codeInfo)
                fileSystem.createDirectories(deployedVersionPath.parent!!)
                fileSystem.write(deployedVersionPath) {
                    writeUtf8(codeInfoJson)
                }
                codeInfo
            }
        }
        outOfDateContractsToVersions[contractType] = ContractCodeMetadata(cargoTomlVersion, codeInfo)
    }
    return outOfDateContractsToVersions
}

suspend fun main() {
    println("Current absolute path is: ${".".toPath().toFile().absolutePath}")
    val nodeInfo = getNode("../integration-tests/src/commonTest/resources/config/nodes.json")
    println(nodeInfo)
    val client = with(nodeInfo) {
        val wallet =
            DirectSigningWallet("sand check forward humble between movie language siege where social crumble mouse") // Use default constructor of wallet to generate random mnemonic.
        SigningCosmWasmClient.init(
            grpcGatewayEndpoint, wallet, chainId = chainId
        )
    }
    val accAddress = (client.wallet as DirectSigningWallet).accounts[0].address
    if (nodeInfo !is Secret4) {
        BalanceUtils.fillUpFromFaucet(nodeInfo, client, 100_000_000, accAddress)
    }

    val storedContracts = storeUpdatedContracts(client, accAddress, nodeInfo)
    println("storedContracts: $storedContracts")
}