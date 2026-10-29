import java.io.File
import java.io.IOException
import java.net.URI
import java.nio.file.AtomicMoveNotSupportedException
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.MessageDigest

plugins {
    java
    id("com.diffplug.spotless") version "8.0.0"
    id("com.gradleup.shadow") version "9.2.2"
}

providers.gradleProperty("axolotlBuildDir").orNull?.let {
    layout.buildDirectory.set(file(it))
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.ow2.asm:asm:9.9")
    implementation("org.ow2.asm:asm-tree:9.9")
    implementation("com.google.code.gson:gson:2.13.2")

    testImplementation(libs.junit.jupiter)
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(17)
    }
}

tasks.withType<JavaCompile>().configureEach {
    options.release = 8
    options.encoding = "UTF-8"
    options.compilerArgs.addAll(listOf("-Xlint:all", "-Werror"))
}

spotless {
    java {
        palantirJavaFormat()
        removeUnusedImports()
    }
}

tasks.jar {
    enabled = false
}

tasks.shadowJar {
    archiveFileName = "theseus.jar"
    manifest {
        attributes["Premain-Class"] = "com.modrinth.theseus.agent.TheseusAgent"
    }

    addMultiReleaseAttribute = false
    enableAutoRelocation = true
    relocationPrefix = "com.modrinth.theseus.shadow"
}

abstract class DownloadPinnedAgent : DefaultTask() {
    @get:Input
    abstract val artifactUrl: Property<String>

    @get:Input
    abstract val expectedSha256: Property<String>

    @get:Input
    abstract val offline: Property<Boolean>

    @get:OutputFile
    abstract val destination: RegularFileProperty

    @get:Internal
    abstract val cacheFile: RegularFileProperty

    init {
        // Validate our content-addressed cache ourselves. Gradle must not delete
        // a valid preseeded output just because task history is unavailable.
        doNotTrackState("The pinned agent is validated by SHA-256 on every invocation")
    }

    private fun checksum(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256")
        .digest(bytes)
        .joinToString("") { "%02x".format(it) }

    private fun readValid(file: File): ByteArray? = try {
        if (file.isFile) file.readBytes().takeIf { checksum(it) == expectedSha256.get() } else null
    } catch (_: IOException) {
        null
    }

    private fun writeAtomically(file: File, bytes: ByteArray) {
        val target = file.toPath()
        Files.createDirectories(target.parent)
        val temporary = Files.createTempFile(target.parent, "authlib-injector-", ".tmp")
        try {
            Files.write(temporary, bytes)
            try {
                Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING)
            } catch (_: AtomicMoveNotSupportedException) {
                Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING)
            }
        } finally {
            Files.deleteIfExists(temporary)
        }
    }

    @TaskAction
    fun acquire() {
        val target = destination.get().asFile
        val cached = cacheFile.get().asFile
        val existing = readValid(target)
        val cachedBytes = readValid(cached)
        val bytes = existing ?: cachedBytes ?: run {
            check(!offline.get()) {
                "authlib-injector is missing or invalid in offline mode. " +
                    "Place the SHA-256 verified agent at $target or $cached, or run once without --offline."
            }
            val connection = URI(artifactUrl.get()).toURL().openConnection().apply {
                connectTimeout = 15_000
                readTimeout = 30_000
            }
            connection.getInputStream().use { it.readBytes() }.also {
                val actual = checksum(it)
                check(actual == expectedSha256.get()) {
                    "authlib-injector checksum mismatch: $actual"
                }
            }
        }
        if (existing == null) writeAtomically(target, bytes)
        if (cachedBytes == null) {
            try {
                writeAtomically(cached, bytes)
            } catch (error: IOException) {
                logger.warn("Could not cache authlib-injector at $cached: ${error.message}")
            }
        }
    }
}

val authlibInjector by tasks.registering(DownloadPinnedAgent::class) {
    artifactUrl.set("https://authlib-injector.yushi.moe/artifact/56/authlib-injector-1.2.8.jar")
    expectedSha256.set("9c7f4343e6c82034958ffb48c14a2cb0c85928be7283103ce17da00c6d5a7b10")
    offline.set(gradle.startParameter.isOffline)
    destination.set(layout.buildDirectory.file("libs/authlib-injector.jar"))
    val cacheDirectory = File(gradle.gradleUserHomeDir, "caches/axolotl/authlib-injector")
    cacheFile.fileProvider(expectedSha256.map {
        File(cacheDirectory, "$it.jar")
    })
}

tasks.build {
    dependsOn(authlibInjector)
}

tasks.named<Test>("test") {
    useJUnitPlatform()
}

tasks.withType<AbstractArchiveTask>().configureEach {
    isPreserveFileTimestamps = false
    isReproducibleFileOrder = true
}
