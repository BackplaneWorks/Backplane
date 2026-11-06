plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Release inputs are explicit. Debug builds keep their integration defaults.
val releaseVersion = providers.environmentVariable("BACKPLANE_ANDROID_VERSION_NAME").orNull
val releaseCodeText = providers.environmentVariable("BACKPLANE_ANDROID_VERSION_CODE").orNull
val releaseCode = releaseCodeText?.toIntOrNull()
require(releaseVersion == null || releaseVersion.matches(Regex("[0-9]+\\.[0-9]+\\.[0-9]+(?:-[0-9A-Za-z.-]+)?"))) {
    "BACKPLANE_ANDROID_VERSION_NAME must be an explicit version"
}
require(releaseCodeText == null || (releaseCode != null && releaseCode in 1..2100000000)) {
    "BACKPLANE_ANDROID_VERSION_CODE must be a positive Android version code"
}
val releaseStorePath = providers.environmentVariable("BACKPLANE_ANDROID_KEYSTORE_PATH").orNull
val releaseStorePassword = providers.environmentVariable("KEYSTORE_PASSWORD").orNull
val releaseKeyAlias = providers.environmentVariable("KEY_ALIAS").orNull
val releaseKeyPassword = providers.environmentVariable("KEY_PASSWORD").orNull
val releaseConfigured = !releaseVersion.isNullOrBlank() && releaseCode != null &&
    !releaseStorePath.isNullOrBlank() && !releaseStorePassword.isNullOrBlank() &&
    !releaseKeyAlias.isNullOrBlank() && !releaseKeyPassword.isNullOrBlank()

gradle.taskGraph.whenReady {
    if (allTasks.any { it.project.path == project.path && it.name.contains("Release") }) {
        require(releaseConfigured) { "Release requires explicit version, counter and configured signing inputs" }
        require(file(releaseStorePath!!).isFile) { "Configured release keystore file is unavailable" }
    }
}

// The GPUI experiment's one build setting: -PbpNative=false gives the
// production behaviour (the WebView shell, a Classic screens switch).
// Default true: native Compose screens, no toggle. Debug builds only.
val bpNative = providers.gradleProperty("bpNative").map { it != "false" }.getOrElse(true)
// -PbpNativeSidebar=false: do not ask the hub for bounded sidebar pages (to test segmented replies); default true
// -PbpE2eHost=host[,host]: the debug build also allows plain http to these fixture hosts (the e2e hub on h@backplane); the release config keeps the shared hub only
val bpE2eHosts = providers.gradleProperty("bpE2eHost").map { v -> v.split(",").map { it.trim() }.filter { it.matches(Regex("[A-Za-z0-9.-]+")) } }.getOrElse(emptyList())
// written while the build is configured (no task, so nothing has to be ordered before the resource merge)
if (bpE2eHosts.isNotEmpty()) {
    val f = layout.buildDirectory.file("generated/e2e-net/res/xml/network.xml").get().asFile
    f.parentFile.mkdirs()
    val extra = bpE2eHosts.joinToString("") { "        <domain includeSubdomains=\"false\">$it</domain>\n" }
    f.writeText("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<network-security-config>\n    <base-config cleartextTrafficPermitted=\"false\" />\n    <domain-config cleartextTrafficPermitted=\"true\">\n        <domain includeSubdomains=\"false\">desktop.tail9e0cf2.ts.net</domain>\n$extra    </domain-config>\n</network-security-config>\n")
}
val bpNativeSidebar = providers.gradleProperty("bpNativeSidebar").map { it != "false" }.getOrElse(true)
// -PbpData=false: the old -bridge launcher; default true: the data/1 app (DataActivity)
val bpData = providers.gradleProperty("bpData").map { it != "false" }.getOrElse(true)
val bpAppId = "dev.backplane.mobile.gpui"
val bpScheme = "bpgpui"

android {
    namespace = "dev.backplane.mobile"
    compileSdk = 35

    defaultConfig {
        applicationId = bpAppId
        manifestPlaceholders["urlScheme"] = bpScheme
        resValue("string", "app_name", "Backplane GPUI")
        resValue("string", "app_id", bpAppId)
        buildConfigField("boolean", "DATA_APP", bpData.toString())
        buildConfigField("boolean", "NATIVE_ONLY", bpNative.toString())
        buildConfigField("boolean", "NATIVE_SIDEBAR", bpNativeSidebar.toString())
        buildConfigField("String", "URL_SCHEME", "\"$bpScheme\"")
        minSdk = 26
        targetSdk = 35
        versionCode = releaseCode ?: 1
        versionName = releaseVersion ?: "0.15.0"
    }

    val configuredRelease = if (releaseConfigured) signingConfigs.create("configuredRelease") {
        storeFile = file(releaseStorePath!!)
        storePassword = releaseStorePassword
        keyAlias = releaseKeyAlias
        keyPassword = releaseKeyPassword
    } else null
    buildTypes {
        release {
            isMinifyEnabled = false
            isDebuggable = false
            signingConfig = configuredRelease
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        buildConfig = true
        compose = true
    }
    // bridge.js is copied in by scripts/build-mobile.sh
    if (bpE2eHosts.isNotEmpty()) {
        sourceSets["debug"].res.srcDir(layout.buildDirectory.dir("generated/e2e-net/res"))
    }
    sourceSets["main"].assets.srcDir("../../build/assets")
    // window.backplaneApp, the same script the iOS app injects
    sourceSets["main"].assets.srcDir("../../shell")
}

dependencies {
    val bom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(bom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("wang.harlon.quickjs:wrapper-android:3.2.3")
    implementation("androidx.javascriptengine:javascriptengine:1.0.0")
    // the WebView shell's bridge (message listener, document-start script)
    implementation("androidx.webkit:webkit:1.12.1")
    // a photo of a pairing code, read on the phone (Pair.kt's pairingFromImage)
    implementation("com.google.zxing:core:3.5.3")
    // the pairing code scanner (Google Play services, no camera permission)
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
    // it brings an old Fragment; the ActivityResult APIs need 1.3 or later
    implementation("androidx.fragment:fragment-ktx:1.8.5")
    testImplementation("junit:junit:4.13.2")
    // the real org.json for unit tests (android.jar's is a stub)
    testImplementation("org.json:json:20240303")
}
