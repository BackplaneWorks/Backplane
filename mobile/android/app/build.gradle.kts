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

android {
    namespace = "dev.backplane.mobile"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.backplane.mobile"
        minSdk = 26
        targetSdk = 35
        versionCode = releaseCode ?: 1
        versionName = releaseVersion ?: "0.1.0"
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
        compose = true
    }
    // bridge.js is copied in by scripts/build-mobile.sh
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
    // a pairing code read on the phone: a photo (Pair.kt's pairingFromImage)
    // and the in-app scanner's camera frames (Scanner.kt)
    implementation("com.google.zxing:core:3.5.3")
    // the in-app scanner's camera (no Google Play services)
    val camerax = "1.4.1"
    implementation("androidx.camera:camera-camera2:$camerax")
    implementation("androidx.camera:camera-lifecycle:$camerax")
    implementation("androidx.camera:camera-view:$camerax")
    testImplementation("junit:junit:4.13.2")
}
