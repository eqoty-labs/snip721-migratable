package io.eqoty.dapp.secret.utils

import io.eqoty.dapp.secret.utils.Constants.GITPOD_ID_ENV_NAME
import io.eqoty.dapp.secret.utils.Constants.TESTNET_TYPE_ENV_NAME
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import okio.Path
import okio.Path.Companion.toPath

@Serializable
sealed interface TestnetInfo {
    val type: String
    val chainId: String
    val grpcGatewayEndpoint: String
    val faucetAddressEndpoint: String
    fun createFaucetAddressGetEndpoint(address: String) = faucetAddressEndpoint + address
}

@Serializable
@SerialName("LocalSecret")
class LocalSecret(
    override val type: String,
    override val chainId: String,
    override val grpcGatewayEndpoint: String,
    override val faucetAddressEndpoint: String,
) : TestnetInfo

@Serializable
@SerialName("Pulsar2")
class Pulsar2(
    override val type: String,
    override val chainId: String,
    override val grpcGatewayEndpoint: String,
    override val faucetAddressEndpoint: String,
) : TestnetInfo

class Gitpod(
    val gitpodId: String,
) : TestnetInfo {
    override val type: String = "Gitpod"
    override val chainId = "secretdev-1"
    override val grpcGatewayEndpoint: String = "https://1317-$gitpodId.gitpod.io"
    override val faucetAddressEndpoint: String =
        "https://5000-$gitpodId.gitpod.io/faucet?address=".replace("\$gitpodId", gitpodId)
}

@Serializable
@SerialName("Custom")
class Custom(
    override val type: String,
    override val chainId: String,
    override val grpcGatewayEndpoint: String,
    override val faucetAddressEndpoint: String,
) : TestnetInfo

@Serializable
data class ConfigTestnets(val testnets: List<TestnetInfo>)

fun getTestnet(): TestnetInfo {
    val testnets: Path = "src/commonTest/resources/config/testnets.json".toPath()
    val jsonString = fileSystem.read(testnets) {
        readUtf8()
    }
    val config: ConfigTestnets = Json.decodeFromString(jsonString)
    val testnetType = getEnv(TESTNET_TYPE_ENV_NAME)
    return if (testnetType == "Gitpod") {
        val gitpodId = try {
            getEnv(GITPOD_ID_ENV_NAME)!!
        } catch (t: Throwable) {
            throw RuntimeException(
                "GITPOD_ID environment variable not found. GITPOD_ID Should be set " +
                        "in local.properties or directly as an environment variable."
            )
        }
        Gitpod(gitpodId)
    } else {
        config.testnets.first { it.type == testnetType }
    }
}
