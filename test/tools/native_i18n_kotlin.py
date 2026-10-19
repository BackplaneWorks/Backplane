#!/usr/bin/env python3
"""Run the catalog smoke test with the project's cached Kotlin compiler (no emulator)."""
from pathlib import Path
import os
import re
import subprocess
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[2]
version = re.search(r'id\("org.jetbrains.kotlin.android"\) version "([^"]+)"', (root / 'mobile/android/build.gradle.kts').read_text())[1]
cache = Path.home() / '.gradle/caches/modules-2/files-2.1'

def artifact(group, name, ver, ext):
    matches = list((cache / group / name / ver).glob(f'*/{name}-{ver}.{ext}'))
    if len(matches) != 1:
        raise SystemExit(f'Need cached {group}:{name}:{ver}; build the Android app first')
    return matches[0]

compiler = artifact('org.jetbrains.kotlin', 'kotlin-compiler-embeddable', version, 'jar')
pom = artifact('org.jetbrains.kotlin', 'kotlin-compiler-embeddable', version, 'pom')
ns = {'m': 'http://maven.apache.org/POM/4.0.0'}
deps = []
for dep in ET.parse(pom).findall('m:dependencies/m:dependency', ns):
    get = lambda key: dep.findtext('m:' + key, namespaces=ns)
    if get('scope') in (None, 'compile', 'runtime'):
        deps.append(artifact(get('groupId'), get('artifactId'), get('version'), 'jar'))
stdlib = artifact('org.jetbrains.kotlin', 'kotlin-stdlib', version, 'jar')
annotations = artifact('org.jetbrains', 'annotations', '13.0', 'jar')
cp = os.pathsep.join(map(str, [compiler, *deps, annotations]))
java = str(Path(os.environ['JAVA_HOME']) / 'bin/java') if 'JAVA_HOME' in os.environ else 'java'
out = root / 'build/native-i18n/kotlin'
out.mkdir(parents=True, exist_ok=True)
subprocess.run([java, '-Xmx2g', '-cp', cp, 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',
                '-no-stdlib', '-no-reflect', '-classpath', os.pathsep.join(map(str, [stdlib, annotations])),
                '-d', str(out), str(root / 'mobile/android/app/src/main/java/dev/backplane/mobile/NativeMessages.kt'),
                str(root / 'test/native/I18nNativeCheck.kt')], check=True)
subprocess.run([java, '-cp', os.pathsep.join(map(str, [out, stdlib])), 'dev.backplane.mobile.I18nNativeCheckKt'], check=True)
