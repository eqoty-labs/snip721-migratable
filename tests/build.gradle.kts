import org.jetbrains.kotlin.gradle.targets.jvm.tasks.KotlinJvmTest
import org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest
import org.jetbrains.kotlin.gradle.tasks.KotlinTest
import java.util.*

plugins {
    @Suppress("DSL_SCOPE_VIOLATION")
    val libs = libs
    alias(libs.plugins.org.jetbrains.kotlin.multiplatform)
    alias(libs.plugins.org.jetbrains.kotlin.plugin.serialization)
}

group = "io.eqoty.dapp"
version = "1.0"

object Targets {
    val iosTargets = arrayOf<String>()
    val macosTargets = arrayOf("macosX64", "macosArm64")
    val darwinTargets = iosTargets + macosTargets
    val linuxTargets = arrayOf<String>()
    val mingwTargets = arrayOf<String>()
    val nativeTargets = linuxTargets + darwinTargets + mingwTargets
}

kotlin {
    jvm()
    for (target in Targets.nativeTargets) {
        targets.add(presets.getByName(target).createTarget(target))
    }
    sourceSets {
        all {
            languageSettings.optIn("kotlinx.coroutines.ExperimentalCoroutinesApi")
            languageSettings.optIn("kotlin.ExperimentalUnsignedTypes")
        }
        val commonMain by getting {
            dependencies {
                // intellij won't recognize FileSystem.SYSTEM in nativeTest
                // for some reason if we declare this in commonTest
                implementation(libs.com.squareup.okio)
            }
        }
        val commonTest by getting {
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.kotlinx.coroutines.core)
                implementation(libs.kotlinx.coroutines.test)
                implementation(libs.kotlinx.serialization.json)
                implementation(libs.ktor.client.core)
                implementation(libs.co.touchlab.kermit)
                implementation(libs.io.eqoty.secretk.client)
            }
        }
        val jvmMain by getting {
            dependsOn(commonMain)
            dependencies {
                implementation(libs.ktor.client.okhttp)
            }
        }
        val jvmTest by getting {
            dependsOn(commonTest)
        }
        val nativeMain by creating {
            dependsOn(commonMain)
        }
        val nativeTest by creating {
            dependsOn(commonTest)
        }
        Targets.nativeTargets.forEach { target ->
            getByName("${target}Main") {
                dependsOn(nativeMain)
            }
            getByName("${target}Test") {
                dependsOn(nativeTest)
            }
        }
    }
}

fun createEnvVariables(environment: Map<String, Any>): MutableMap<String, Any> {
    val envMap = mutableMapOf<String, Any>()
    envMap.putAll(environment)
    if (envMap["TESTNET_TYPE"] == null) {
        val properties = Properties()
        properties.load(project.rootProject.file("gradle.properties").reader())
        val localPropertiesFile = project.rootProject.file("local.properties")
        if(localPropertiesFile.exists()) {
            properties.load(localPropertiesFile.reader())
        }
        val testnetType = properties["TESTNET_TYPE"]
        val gitpodId = properties["GITPOD_ID"]
        val contractPath = properties["CONTRACT_PATH"]
        envMap["TESTNET_TYPE"] = testnetType!!
        envMap["CONTRACT_PATH"] = contractPath!!
        gitpodId?.let {
            envMap.put("GITPOD_ID", it)
        }
    }
    return envMap
}

tasks.withType<Test> {
    environment = createEnvVariables(environment)
    testLogging {
        showStandardStreams = true
    }
}

tasks.withType<KotlinNativeTest> {
    environment = createEnvVariables(environment)
    testLogging {
        showStandardStreams = true
    }
}
