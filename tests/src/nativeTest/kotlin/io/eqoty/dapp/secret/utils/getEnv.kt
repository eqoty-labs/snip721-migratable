package io.eqoty.dapp.secret.utils

import platform.posix.getenv

actual fun getEnv(name: String): String? = getenv(name)?.toString()

