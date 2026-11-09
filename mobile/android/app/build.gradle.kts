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

// -Pbp.appId=… -Pbp.scheme=…: a test build that installs beside the one a phone already uses (it keeps its pairing)
val bpAppId = (findProperty("bp.appId") as String?) ?: "dev.backplane.mobile.gpui"
val bpScheme = (findProperty("bp.scheme") as String?) ?: "bpgpui"
// -Pbp.appName=…: the launcher name (the release is "Backplane", as 0.14.1 was)
val bpAppName = (findProperty("bp.appName") as String?) ?: "Backplane GPUI"

android {
    namespace = "dev.backplane.mobile"
    compileSdk = 35

    defaultConfig {
        applicationId = bpAppId
        manifestPlaceholders["urlScheme"] = bpScheme
        resValue("string", "app_name", bpAppName)
        resValue("string", "app_id", bpAppId)
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
}

// The app's copy comes from i18n/en.msg and ja.msg (Msg.kt, scripts/i18n-android.py): every build checks that Msg.kt is current and that no
// bare English reaches the screen (scripts/check-android-i18n.py), so a stale catalog or a hard-coded label fails here, in CI and on a desktop alike.
val i18nCheck = tasks.register<Exec>("i18nCheck") {
    val root = rootProject.projectDir.parentFile.parentFile
    inputs.files(fileTree(root.resolve("i18n")), fileTree(projectDir.resolve("src/main/java")), root.resolve("mobile/android/i18n-allow.txt"))
    outputs.upToDateWhen { false }
    workingDir = root
    commandLine("sh", "-c", "python3 scripts/i18n-check.py && python3 scripts/i18n-android.py --check && python3 scripts/check-android-i18n.py")
}
tasks.named("preBuild") { dependsOn(i18nCheck) }

dependencies {
    val bom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(bom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    // a pairing code read on the phone: a photo (Pair.kt's pairingFromImage) and the in-app scanner's camera frames (Scanner.kt)
    implementation("com.google.zxing:core:3.5.3")
    // the in-app scanner's camera (no Google Play services)
    val camerax = "1.4.1"
    implementation("androidx.camera:camera-camera2:$camerax")
    implementation("androidx.camera:camera-lifecycle:$camerax")
    implementation("androidx.camera:camera-view:$camerax")
    testImplementation("junit:junit:4.13.2")
    // the real org.json for unit tests (android.jar's is a stub)
    testImplementation("org.json:json:20240303")
}
