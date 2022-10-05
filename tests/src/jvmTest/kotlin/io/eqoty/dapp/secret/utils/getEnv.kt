package io.eqoty.dapp.secret.utils

actual fun getEnv(name: String): String? {
    val a = System.getenv(name)
    println("getEnv $name:$a")
    return a
}

