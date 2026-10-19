plugins {
    id("com.android.application")
}

android {
    namespace = "dev.voiceo.app"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.voiceo.app"
        minSdk = 29
        targetSdk = 35
        versionCode = (System.getenv("VOICEO_VERSION_CODE") ?: "1").toInt()
        versionName = System.getenv("VOICEO_VERSION_NAME") ?: "0.1.0"
        ndk { abiFilters += "arm64-v8a" }
    }

    val ksFile = System.getenv("VOICEO_KEYSTORE")
    signingConfigs {
        if (ksFile != null) create("release") {
            storeFile = file(ksFile)
            storePassword = System.getenv("VOICEO_KEYSTORE_PASSWORD")
            keyAlias = System.getenv("VOICEO_KEY_ALIAS")
            keyPassword = System.getenv("VOICEO_KEY_PASSWORD")
            enableV1Signing = true
            enableV2Signing = true
            enableV3Signing = true
        }
    }
    buildTypes {
        release {
            if (ksFile != null) signingConfig = signingConfigs.getByName("release")
            isMinifyEnabled = false
        }
    }

    // The settings UI and glossary data are shared with the desktop app (src/, src-tauri/data/).
    sourceSets {
        getByName("main") {
            assets.directories.add("../../src")
            assets.directories.add("../../src-tauri/data")
        }
    }
    androidResources {
        ignoreAssetsPattern = "!atspi_helper.py:!*.md:!icon*"
    }

    buildFeatures { buildConfig = true }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.19.1")
    testImplementation("junit:junit:4.13.2")
}
