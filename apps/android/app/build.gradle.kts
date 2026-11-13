import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

/** The repository's root (the Cargo workspace). */
val repoRoot: File = rootProject.projectDir.resolve("../..").canonicalFile

/** The app's version: the workspace's (Cargo.toml), as everywhere else. */
val appVersion: String = run {
    val text = repoRoot.resolve("Cargo.toml").readText()
    Regex("""\[workspace\.package\][^\[]*?version\s*=\s*"([^"]+)"""").find(text)!!.groupValues[1]
}

/** As Tauri numbered its Android builds (so that this one installs over them). */
val appVersionCode: Int = run {
    val (major, minor, patch) = appVersion.split("-")[0].split(".").map { it.toInt() }
    major * 1_000_000 + minor * 1_000 + patch
}

val isRelease = gradle.startParameter.taskNames.any { it.contains("release", ignoreCase = true) }

/** The ABIs the engine is built for: `-Pnisshoku.abis=x86_64` for the emulator alone. */
val abis: List<String> = (findProperty("nisshoku.abis") as String? ?: "arm64-v8a,armeabi-v7a,x86_64").split(",")

android {
    namespace = "app.nisshoku.android"
    compileSdk = 37
    defaultConfig {
        // The same app as the Tauri build it replaces: installs over it, keeps its profile.
        applicationId = "app.nisshoku.desktop"
        minSdk = 26
        targetSdk = 37
        versionCode = appVersionCode
        versionName = appVersion
        ndk { abiFilters += abis }
    }
    val keystoreFile = rootProject.file("keystore.properties")
    if (keystoreFile.exists()) {
        val ks = Properties().apply { keystoreFile.inputStream().use { load(it) } }
        signingConfigs {
            create("release") {
                storeFile = file(ks.getProperty("storeFile"))
                storePassword = ks.getProperty("storePassword")
                keyAlias = ks.getProperty("keyAlias")
                keyPassword = ks.getProperty("keyPassword")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-dev"
        }
        getByName("release") {
            signingConfigs.findByName("release")?.let { signingConfig = it }
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    sourceSets {
        getByName("main") {
            kotlin.srcDir(file("build/generated/uniffi"))
        }
    }
    packaging {
        jniLibs.keepDebugSymbols += "**/*.so"
    }
}

// -------------------------------------------------------------------------
// The engine (crates/nisshoku-mobile), built with cargo-ndk, and its Kotlin
// bindings (UniFFI, read from the built library).

val jniLibsDir = file("src/main/jniLibs")

val cargoBuild = tasks.register<Exec>("cargoBuild") {
    group = "nisshoku"
    description = "Builds the engine for Android"
    workingDir = repoRoot
    val args = mutableListOf("cargo", "ndk", "--platform", "26", "-o", jniLibsDir.path)
    abis.forEach { args += listOf("-t", it) }
    args += listOf("build", "-p", "nisshoku-mobile")
    if (isRelease) args += "--release"
    commandLine(args)
    inputs.dir(repoRoot.resolve("crates"))
    inputs.file(repoRoot.resolve("Cargo.lock"))
    outputs.dir(jniLibsDir)
}

val uniffiBindings = tasks.register<Exec>("uniffiBindings") {
    group = "nisshoku"
    description = "Writes the Kotlin bindings of the engine"
    dependsOn(cargoBuild)
    workingDir = repoRoot
    val out = layout.buildDirectory.dir("generated/uniffi").get().asFile
    val library = jniLibsDir.resolve("${abis.first()}/libnisshoku_mobile.so")
    commandLine(
        "cargo", "run", "-q", "-p", "nisshoku-mobile", "--features", "bindgen", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", library.path, "--language", "kotlin", "--out-dir", out.path, "--no-format",
    )
    inputs.file(library)
    outputs.dir(out)
}

// -------------------------------------------------------------------------
// The editor: the web app's (apps/desktop), shown alone (`?embed=editor`).

val webDir = repoRoot.resolve("apps/desktop")

val webBuild = tasks.register<Exec>("webBuild") {
    group = "nisshoku"
    description = "Builds the web editor"
    workingDir = webDir
    val windows = System.getProperty("os.name").lowercase().contains("windows")
    commandLine(if (windows) listOf("cmd", "/c", "pnpm", "build") else listOf("pnpm", "build"))
    inputs.dir(webDir.resolve("src"))
    inputs.file(webDir.resolve("index.html"))
    outputs.dir(webDir.resolve("dist"))
    onlyIf { findProperty("nisshoku.skipWeb") == null }
}

val webAssets = tasks.register<Sync>("webAssets") {
    group = "nisshoku"
    dependsOn(webBuild)
    from(webDir.resolve("dist"))
    into(file("src/main/assets/web"))
}

tasks.named("preBuild") { dependsOn(uniffiBindings, webAssets) }

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.compose.material3:material3:1.5.0-rc01")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.animation:animation")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("androidx.core:core-splashscreen:1.2.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-process:2.11.0")
    implementation("androidx.navigation:navigation-compose:2.10.2")
    implementation("androidx.webkit:webkit:1.18.0")
    implementation("androidx.work:work-runtime-ktx:2.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.13.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    // UniFFI's Kotlin bindings call the engine through JNA.
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.rustls:rustls-platform-verifier:$rustlsPlatformVerifierVersion")
}

/** The Kotlin half of rustls-platform-verifier, at the version of its crate in Cargo.lock. */
val rustlsPlatformVerifierVersion: String
    get() {
        val crate = "rustls-platform-verifier-android"
        val lines = repoRoot.resolve("Cargo.lock").readLines()
        val index = lines.indexOfFirst { it.trim() == "name = \"$crate\"" }
        require(index >= 0) { "$crate not found in Cargo.lock" }
        return Regex("\"([^\"]+)\"").find(lines[index + 1])!!.groupValues[1]
    }
