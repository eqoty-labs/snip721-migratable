package io.eqoty.dapp.secret.utils

import io.ktor.util.reflect.*
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import okio.Path
import okio.Path.Companion.toPath

@Serializable
sealed interface TestnetInfo {
    val chainId: String
    val grpcGatewayEndpoint: String
    val faucetAddressEndpoint: String
    fun createFaucetAddressGetEndpoint(address: String) = faucetAddressEndpoint + address
}

@Serializable
@SerialName("LocalSecret")
class LocalSecret(
    override val chainId: String,
    override val grpcGatewayEndpoint: String,
    override val faucetAddressEndpoint: String,
) : TestnetInfo

@Serializable
@SerialName("Pulsar2")
class Pulsar2(
    override val chainId: String,
    override val grpcGatewayEndpoint: String,
    override val faucetAddressEndpoint: String,
) : TestnetInfo

@Serializable
@SerialName("Gitpod")
class Gitpod(
    override val chainId: String,
    val gitpodId: String,
) : TestnetInfo {
    override val grpcGatewayEndpoint: String = "https://1317-$gitpodId.gitpod.io"
    override val faucetAddressEndpoint: String =
        "https://5000-$gitpodId.gitpod.io/faucet?address=".replace("\$gitpodId", gitpodId)
}

@Serializable
@SerialName("Custom")
class Custom(
    override val chainId: String,
    override val grpcGatewayEndpoint: String,
    override val faucetAddressEndpoint: String,
) : TestnetInfo

@Serializable
data class ConfigTestnets(val testnets: List<TestnetInfo>)

val selectedLocalRunTestnet = Gitpod::class

fun getTestnet(): TestnetInfo {
    val testnets: Path = "src/commonTest/resources/config/testnets.json".toPath()
    val jsonString = fileSystem.read(testnets) {
        readUtf8()
    }
    val config: ConfigTestnets = Json.decodeFromString(jsonString)
    val isRunningOnCI = fileSystem.exists("isRunningOnCI".toPath())
    return if (isRunningOnCI) {
        config.testnets.first { it is LocalSecret }
    } else {
        config.testnets.first { it.instanceOf(selectedLocalRunTestnet) }
    }
}
