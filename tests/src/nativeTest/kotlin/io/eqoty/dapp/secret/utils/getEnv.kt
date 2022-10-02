package io.eqoty.dapp.secret.utils

import kotlinx.cinterop.toKString
import platform.posix.getenv

actual fun getEnv(name: String): String? = getenv(name)?.toKString()

