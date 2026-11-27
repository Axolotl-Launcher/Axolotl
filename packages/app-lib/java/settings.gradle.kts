pluginManagement {
    repositories {
        providers.environmentVariable("AXOLOTL_MAVEN_CENTRAL_MIRROR").orNull?.let { mirror ->
            maven { url = uri(mirror) }
        }
        gradlePluginPortal()
    }
}

plugins {
    // Apply the foojay-resolver plugin to allow automatic download of JDKs
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

rootProject.name = "theseus"
