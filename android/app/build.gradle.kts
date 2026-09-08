import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    kotlin("android")
}

val repoRoot = rootProject.projectDir.parentFile
val rustBuildScript = repoRoot.resolve("scripts/android-build-native.sh")
val rustJniLibsDir = project.layout.projectDirectory.dir("src/main/jniLibs")

android {
    namespace = "dev.suzaku.android.ime"
    compileSdk = 35
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "dev.suzaku.android.ime"
        minSdk = 29
        targetSdk = 35
        versionCode = 4
        versionName = "0.5.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        viewBinding = true
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir(rustJniLibsDir)
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

val buildRustAndroidLibs by tasks.registering(Exec::class) {
    group = "build"
    description = "Build Rust Android shared libraries and copy them into jniLibs."
    workingDir = repoRoot
    commandLine("bash", rustBuildScript.absolutePath)
    environment("SUZAKU_ANDROID_PROFILE", "debug")
    onlyIf { rustBuildScript.isFile }
}

tasks.matching { it.name == "preBuild" }.configureEach {
    dependsOn(buildRustAndroidLibs)
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    testImplementation("junit:junit:4.13.2")
}
