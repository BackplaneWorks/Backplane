"""Offline native model checks. Uses cached compilers; never installs dependencies."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
ANDROID = ROOT / "mobile/android/app/src/main/java/dev/backplane/mobile"
IOS = ROOT / "mobile/ios/Backplane"
JAVA = str(Path(os.environ["JAVA_HOME"]) / "bin/java") if os.environ.get("JAVA_HOME") else shutil.which("java")

def run(args):
    subprocess.run([str(a) for a in args], check=True, cwd=ROOT)

with tempfile.TemporaryDirectory(prefix="improvements-tests-") as directory:
    out = Path(directory)
    swift = shutil.which("swiftc")
    if swift:
        run([swift, IOS / "ImprovementsDraft.swift", HERE / "main.swift", "-o", out / "ios-test"])
        run([out / "ios-test"])
    else:
        print("SKIP iOS executable checks: swiftc unavailable (SwiftUI/device checks also require Xcode).", flush=True)

    cache = Path(os.environ.get("GRADLE_USER_HOME", Path.home() / ".gradle")) / "caches/modules-2/files-2.1"
    coordinates = [
        "org.jetbrains.kotlin/kotlin-compiler-embeddable/2.0.21",
        "org.jetbrains.kotlin/kotlin-stdlib/2.0.21",
        "org.jetbrains.kotlin/kotlin-script-runtime/2.0.21",
        "org.jetbrains.kotlin/kotlin-reflect/2.0.21",
        "org.jetbrains.intellij.deps/trove4j/1.0.20200330",
        "org.jetbrains.kotlinx/kotlinx-coroutines-core-jvm/1.6.4",
        "org.jetbrains/annotations/13.0",
    ]
    jars = [next((cache / c).glob("*/*.jar"), None) for c in coordinates]
    if JAVA and all(jars):
        cp = os.pathsep.join(str(j) for j in jars)
        run([JAVA, "-Xmx512m", "-cp", cp, "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler", "-no-stdlib", "-no-reflect",
             "-classpath", cp, "-d", out / "android-tests.jar", ANDROID / "ImprovementsDraft.kt", HERE / "ImprovementsTest.kt"])
        run([JAVA, "-cp", str(out / "android-tests.jar") + os.pathsep + cp, "dev.backplane.mobile.ImprovementsTestKt"])
    else:
        raise SystemExit("Cached Kotlin 2.0.21 compiler dependencies or Java unavailable; nothing installed.")
