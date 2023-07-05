import java.util.*

plugins {
    alias(libs.plugins.org.jetbrains.kotlin.multiplatform)
    alias(libs.plugins.org.jetbrains.kotlin.plugin.serialization)
}


kotlin {
    jvm() {
        withJava()
    }
    sourceSets {
        all {
            languageSettings.optIn("kotlinx.coroutines.ExperimentalCoroutinesApi")
            languageSettings.optIn("kotlinx.serialization.ExperimentalSerializationApi")
            languageSettings.optIn("kotlin.ExperimentalUnsignedTypes")
        }
        val commonMain by getting {
            dependencies {
                implementation(libs.com.squareup.okio)
                implementation(libs.io.eqoty.secretk.deploy.utils)
                implementation(libs.io.eqoty.secretk.secret.std.msgs)
                implementation(libs.kotlinx.serialization.json)
                implementation(libs.io.eqoty.secretk.client)
            }
        }
    }
}


fun createEnvVariables(environment: Map<String, Any>): MutableMap<String, Any> {
    val envMap = mutableMapOf<String, Any>()
    envMap.putAll(environment)
    val properties = Properties()
    properties.load(project.rootProject.file("./deploy/deploy.default.properties").reader())
    val localPropertiesFile = project.rootProject.file("local.properties")
    if (localPropertiesFile.exists()) {
        properties.load(localPropertiesFile.reader())
    }
    println(properties)
    if (envMap["NODE_TYPE"] == null) {
        envMap["NODE_TYPE"] = properties["NODE_TYPE"]!!
    }
    properties["GITPOD_ID"]?.let {
        envMap.put("GITPOD_ID", it)
    }
    properties["CONTRACT_PATH"]?.let {
        envMap.put("CONTRACT_PATH", it)
    }
    return envMap
}

tasks.create<JavaExec>("run") {
    environment = createEnvVariables(environment)
    mainClass.set("io.eqoty.dapp.secret.MainKt")
    classpath = sourceSets["main"].runtimeClasspath
}
