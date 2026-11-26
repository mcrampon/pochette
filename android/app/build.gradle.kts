import java.util.Properties
import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

/** The repository, with the Rust workspace. */
val repository: File = rootDir.parentFile

/** The app's version, in gradle.properties: its own, not the desktop's. */
val pochetteVersion = property("pochette.version") as String

/** The ABIs to build the core for: `-Ppochette.abis=x86_64` for the emulator alone. */
val abis: List<String> =
    (findProperty("pochette.abis") as String?)?.split(",")
        ?: listOf("arm64-v8a", "armeabi-v7a", "x86_64")

android {
    namespace = "io.github.mcrampon.pochette"
    compileSdk = 37
    // As the dev shell has them: see flake.nix. AGP strips the core with the NDK's tools.
    buildToolsVersion = "37.0.0"
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "io.github.mcrampon.pochette"
        minSdk = 29
        targetSdk = 36
        // Only the core's ABIs: JNA has its own part for more.
        ndk { abiFilters += abis }
        versionName = pochetteVersion
        // 1.2.3 is 10203: each part up to 99.
        versionCode =
            pochetteVersion.split(".").map(String::toInt).fold(0) { code, part -> code * 100 + part }
    }

    signingConfigs {
        // The release key, kept by whoever releases, out of the repository: a properties file
        // (storeFile, storePassword, keyAlias, keyPassword) that `pochette.signing` names, as in
        // ~/.gradle/gradle.properties. Without it the release is unsigned, as F-Droid builds it
        // before signing it.
        (findProperty("pochette.signing") as String?)?.let { path ->
            val signing = Properties().apply { file(path).inputStream().use(::load) }
            create("release") {
                storeFile = file(signing.getProperty("storeFile"))
                storePassword = signing.getProperty("storePassword")
                keyAlias = signing.getProperty("keyAlias")
                keyPassword = signing.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
            // Smaller, and Compose runs faster once optimized: see proguard-rules.pro for what stays.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // The same APK from the same sources, wherever it's built: see reproducible builds.
            vcsInfo.include = false
        }
    }

    dependenciesInfo {
        // Google's encrypted list of the dependencies, which F-Droid refuses.
        includeInApk = false
        includeInBundle = false
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    testOptions {
        unitTests.all { test ->
            // The core, built for this computer, for the tests on the JVM.
            test.systemProperty("jna.library.path", repository.resolve("target/debug").path)
            test.dependsOn("rustHost")
        }
    }
}

/** Runs Cargo in the repository: it knows itself what to build again. */
abstract class Cargo : DefaultTask() {
    @get:Input
    abstract val arguments: ListProperty<String>

    @get:Internal
    abstract val workspace: DirectoryProperty

    @get:OutputDirectory
    @get:Optional
    abstract val outputDir: DirectoryProperty

    @get:Input
    @get:Optional
    abstract val rustFlags: Property<String>

    @get:Inject
    abstract val execOperations: ExecOperations

    init {
        outputs.upToDateWhen { false }
    }

    companion object {
        /** In the arguments, the output directory. */
        const val OUT = "{out}"
    }

    @TaskAction
    fun run() {
        // Where the output goes, filled in now: AGP picks it once the task is set up.
        val out = outputDir.orNull?.asFile?.path
        execOperations.exec {
            workingDir(workspace.get().asFile)
            rustFlags.orNull?.let { environment("RUSTFLAGS", it) }
            commandLine(listOf("cargo") + arguments.get().map { if (it == OUT) out!! else it })
        }
    }
}

/** The core for Android, for each ABI: always optimized, a debug build is 140 MB. */
val rustAndroid = tasks.register<Cargo>("rustAndroid") {
    workspace.set(repository)
    outputDir.set(layout.buildDirectory.dir("rust/jniLibs"))
    arguments.set(
        listOf("ndk") + abis.flatMap { listOf("-t", it) } +
            listOf("--platform", "29", "-o", Cargo.OUT, "build", "-p", "pochette-ffi", "--release", "--locked"),
    )
    // The same library wherever it's built: no paths of this computer in it. Rust names its standard
    // library's sources /rustc/<commit>, unless they're installed, as for an editor: then it names
    // them where they are, so they're named back.
    val cargoHome = System.getenv("CARGO_HOME") ?: "${System.getProperty("user.home")}/.cargo"
    val sysroot = providers.exec { commandLine("rustc", "--print", "sysroot") }.standardOutput.asText
    val commit =
        providers.exec { commandLine("rustc", "-vV") }.standardOutput.asText.map { info ->
            Regex("(?m)^commit-hash: (\\S+)").find(info)!!.groupValues[1]
        }
    rustFlags.set(
        sysroot.zip(commit) { root, hash ->
            listOf(
                "--remap-path-prefix=${repository.path}=/pochette",
                "--remap-path-prefix=$cargoHome=/cargo",
                "--remap-path-prefix=${root.trim()}/lib/rustlib/src/rust=/rustc/$hash",
            ).joinToString(" ")
        },
    )
}

/** The core for this computer: the bindings are generated from it, and the tests load it. */
val rustHost = tasks.register<Cargo>("rustHost") {
    workspace.set(repository)
    arguments.set(listOf("build", "-p", "pochette-ffi", "--locked"))
}

/** The Kotlin bindings to the core, which UniFFI generates from the library. */
val bindings = tasks.register<Cargo>("bindings") {
    dependsOn(rustHost)
    workspace.set(repository)
    outputDir.set(layout.buildDirectory.dir("generated/uniffi"))
    arguments.set(
        listOf(
            "run", "-p", "pochette-ffi", "--features", "bindgen", "--bin", "uniffi-bindgen", "--",
            "generate", "--library", repository.resolve("target/debug/libpochette_ffi.so").path,
            "--language", "kotlin", "--no-format", "--out-dir", Cargo.OUT,
        ),
    )
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(rustAndroid, Cargo::outputDir)
        variant.sources.kotlin?.addGeneratedSourceDirectory(bindings, Cargo::outputDir)
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.browser)
    implementation(libs.androidx.core)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.coil.compose)
    implementation(libs.coil.network)
    implementation(libs.coroutines.android)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.session)
    implementation(libs.work)
    // What the bindings call the core through: the Android build, with its native part.
    implementation(libs.jna) { artifact { type = "aar" } }

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    // The JVM build, with the native part for this computer.
    testImplementation(libs.jna)
}
