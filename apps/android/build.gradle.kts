plugins {
    id("com.android.application") version "9.3.1" apply false
    // AGP compiles Kotlin itself; this puts the Kotlin version beside it.
    id("org.jetbrains.kotlin.android") version "2.4.21" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.4.21" apply false
}
