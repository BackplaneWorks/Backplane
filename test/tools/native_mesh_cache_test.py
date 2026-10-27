"""Compile/run the actual pure Kotlin mesh decoder/cache without an Android UI.

Uses the already cached Kotlin compiler; writes only a temporary test folder.
No copied implementation or regex-based behavioral assertions stand in for code.
"""
import base64
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / 'mobile/android/app/src/main/java/dev/backplane/mobile/Plot.kt').read_text()
fixture = json.loads((ROOT / 'test/fixtures/solid-handedness/wire.json').read_text())
pure = 'import kotlin.math.sqrt\n'
pure += source[source.index('class Varints('):source.index('// One chunk, decoded once:')]
pure += source[source.index('class PlotMesh('):source.index('// What the renderer draws:')]
pure += source[source.index('class MeshFrame('):source.index('class PlotStore {')]
pure += '''
fun main(args: Array<String>) {
    val bytes = java.util.Base64.getDecoder().decode(args[0])
    val n = args[1].toLong()
    val o = mapOf("n" to n, "mesh" to bytes, "box3" to listOf(1004L,1004L,4294967288L,6998L,5000L,159L))
    val mesh = PlotMesh.decode(o)
    check(mesh.colors.size == n.toInt() * 3)
    check(mesh.box[2] == -80f) { "U32 wire bounds must become signed micrometres" }
    check(mesh.verts.all { it.isFinite() })
    check(runCatching { PlotMesh.decode(o) { true } }.exceptionOrNull() is java.util.concurrent.CancellationException)
    for (bad in listOf(mapOf("n" to 1L, "mesh" to ByteArray(0)), mapOf("n" to Int.MAX_VALUE.toLong(), "mesh" to bytes),
        mapOf("n" to 1L, "mesh" to ByteArray(10) { 255.toByte() }), mapOf("n" to n, "mesh" to bytes.copyOf(bytes.size - 1))))
        check(runCatching { PlotMesh.decode(bad) }.isFailure) { "Malformed/oversized model accepted" }
    val cache = PlotMeshAssets()
    val epoch = cache.epoch()
    val a = cache.issue("board", epoch)!!
    cache.put("board", n.toInt(), bytes, mesh, a)
    check(cache.same("board", n.toInt(), bytes.copyOf()) === mesh)
    val changed = bytes.copyOf(); changed[0] = (changed[0].toInt() xor 1).toByte()
    check(cache.same("board", n.toInt(), changed) == null)
    val b = cache.issue("board", epoch)!!
    check(!cache.current("board", a) && cache.current("board", b))
    val selected = cache.issue("new selected source", epoch)!!
    check(!cache.current("board", b) && cache.current("new selected source", selected)) { "Switch A to B did not cancel A" }
    check(cache.frame("board")?.mesh === mesh) { "Selection switch must keep completed assets for warm reopen" }
    cache.reset()
    check(!cache.current("board", b) && cache.frame("board") == null)
    check(cache.issue("board", epoch) == null) { "A queued message from a retired hub issued a current ticket" }
    cache.put("board", n.toInt(), bytes, mesh, b)
    check(cache.frame("board") == null) { "Late result resurrected retired cache" }
    for (i in 0..4) { val t = cache.issue("part$i", cache.epoch())!!; cache.put("part$i", n.toInt(), bytes, mesh, t) }
    check(cache.frame("part0") == null && cache.frame("part4") != null)
    println("ok actual Kotlin decoder: reviewed board, signed bounds, finite geometry, cancellation, malformed/oversized payloads")
    println("ok actual Kotlin cache: exact wire revision, reset/late result, retired hub message, bounded eviction")
}
'''
cache = Path.home() / '.gradle/caches/modules-2/files-2.1'
coordinates = [
    ('org.jetbrains.kotlin', 'kotlin-compiler-embeddable', '2.0.21'),
    ('org.jetbrains.kotlin', 'kotlin-stdlib', '2.0.21'),
    ('org.jetbrains.kotlin', 'kotlin-script-runtime', '2.0.21'),
    ('org.jetbrains.kotlin', 'kotlin-reflect', '1.6.10'),
    ('org.jetbrains.kotlin', 'kotlin-daemon-embeddable', '2.0.21'),
    ('org.jetbrains.intellij.deps', 'trove4j', '1.0.20200330'),
    ('org.jetbrains', 'annotations', '13.0'),
    ('org.jetbrains.kotlinx', 'kotlinx-coroutines-core-jvm', '1.6.4'),
]
jars = []
for group, artifact, version in coordinates:
    found = list((cache / group / artifact / version).glob('*/*.jar'))
    if not found:
        raise SystemExit(f'cached compiler dependency missing: {group}:{artifact}:{version}')
    jars.append(found[0])
classpath = os.pathsep.join(map(str, jars))
java = os.environ.get('JAVA_HOME', '/usr/lib/jvm/java-17-openjdk-amd64') + '/bin/java'
with tempfile.TemporaryDirectory(prefix='bp-native-mesh-test-') as folder:
    path = Path(folder) / 'NativeMeshTest.kt'; path.write_text(pure)
    out = Path(folder) / 'classes'
    subprocess.run([java, '-Xmx768m', '-cp', classpath, 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',
        '-no-stdlib', '-no-reflect', '-classpath', classpath, '-d', str(out), str(path)], check=True, timeout=90)
    subprocess.run([java, '-cp', str(out) + os.pathsep + classpath, 'NativeMeshTestKt', fixture['mesh_base64'], str(fixture['n'])], check=True, timeout=20)
