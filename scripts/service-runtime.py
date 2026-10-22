#!/usr/bin/env python3
"""Measure a matching host's system ABI for a JS service BUILD (Bun), then assemble its component."""
import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


# Windows system DLLs a service may import; API sets (api-ms-win-*, ext-ms-win-*)
# are OS-owned virtual names resolved by the loader.
SYSTEM_DLLS = set(('ntdll kernel32 kernelbase user32 gdi32 gdi32full shell32 '
                  'advapi32 ole32 oleaut32 bcrypt bcryptprimitives crypt32 '
                  'ws2_32 wsock32 userenv dbghelp iphlpapi imm32 comdlg32 '
                  'comctl32 winmm version setupapi secur32 shlwapi rpcrt4 '
                  'msvcrt ucrtbase normaliz dnsapi netapi32 psapi wtsapi32 '
                  'winhttp wininet powrprof cfgmgr32 dwmapi uxtheme').split())
APISET = re.compile(r'(api|ext)-ms-win-[a-z0-9.-]+\.dll\Z')
# Apple system libraries Bun links against; all live in the dyld shared cache.
APPLE_SYSTEM = {'/usr/lib/libSystem.B.dylib', '/usr/lib/libc++.1.dylib',
                '/usr/lib/libicucore.A.dylib', '/usr/lib/libresolv.9.dylib'}
# TODO (fast-hub L1/J4): measure these from the running service on each
# target instead of declaring them (docs/service-runtime.md).
# What the JS service may dlopen through bun:ffi, per platform. The Bend JS
# runtime (pinned bend) and the server's effects (src/**/*.js, checked by
# ffi_libraries) open the C library and nothing else.
FFI_APPROVED = {'linux': {'libc.so.6'}, 'darwin': {'libSystem.dylib', '/usr/lib/libSystem.B.dylib'}, 'windows': {'kernel32.dll'}}
BEND_RUNTIME_FFI = {'linux': ['libc.so.6'], 'darwin': ['libSystem.dylib'], 'windows': []}


def target_host():
    return {('Linux', 'x86_64'): 'linux-x64', ('Linux', 'aarch64'): 'linux-arm64',
            ('Darwin', 'arm64'): 'darwin-arm64', ('Darwin', 'x86_64'): 'darwin-x64',
            ('Windows', 'AMD64'): 'windows-x64'}.get((platform.system(), platform.machine()))


def family(target):
    return target.split('-')[0]


def ffi_libraries(root, files, target):
    # A service JS source that calls dlopen declares the libraries it may open
    # as string literals in the file (an effect picks one per platform into a
    # variable, `ffi.dlopen(lib, ...)`); a file that calls dlopen and names
    # none is refused, and those named for this platform must be approved
    # system libraries.
    found = {}
    for name in files:
        if not (name.startswith('src/') and name.endswith('.js')):
            continue
        text = (Path(root)/name).read_text(errors='strict')
        if not re.search(r'\bdlopen\(', text):
            continue
        names = sorted({n for n in re.findall(r'["\']([^"\'\s]+)["\']', text)
                        if re.search(r'\.(?:so(?:\.[0-9]+)*|dylib|dll)\Z', n)})
        require(names, 'Dynamic load of an undeclared library: ' + name)
        for lib in names:
            kind = 'linux' if re.search(r'\.so(?:\.[0-9]+)*\Z', lib) else 'darwin' if lib.endswith('.dylib') else 'windows'
            if kind == family(target):
                found.setdefault(lib, set()).add(name)
    for lib in BEND_RUNTIME_FFI[family(target)]:
        found.setdefault(lib, set()).add('Bend JS runtime (pinned bend)')
    require(set(found) <= FFI_APPROVED[family(target)], 'Unapproved dynamically loaded library')
    return [{'name': lib, 'declaredBy': sorted(by)} for lib, by in sorted(found.items())]


class Capture:
    def __init__(self, root):
        self.root, self.calls = root, []

    def __call__(self, argv):
        return self.run(argv)

    def elf_list(self, loader, binary):
        # Fixed measurement-only diagnostic, never caller-provided LD_* values.
        return self.run([loader, '--list', binary], loader_files=True)

    def run(self, argv, loader_files=False):
        # No inherited loader injection, protected bake variables or user tool config.
        env = {k: v for k, v in os.environ.items() if not k.startswith(
            ('LD_', 'DYLD_', 'BACKPLANE_GOOGLE_', 'BACKPLANE_TELEMETRY_'))}
        env.update(LC_ALL='C', LANG='C')
        if loader_files:
            env['LD_DEBUG'] = 'files'
        p = Path(argv[0]).resolve(strict=True)
        result = subprocess.run([str(x) for x in argv], stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE if loader_files else subprocess.STDOUT,
            env=env, timeout=30, close_fds=True)
        text = result.stdout.decode('utf-8', errors='strict')
        name = 'operation-' + str(len(self.calls)) + '.log'
        (self.root/name).write_text(text)
        self.calls.append({'argv': [str(x) for x in argv], 'exit': result.returncode,
            'tool': {'path': str(p), 'sha256': sha(p), 'bytes': p.stat().st_size}, 'log': name})
        if loader_files:
            debug = result.stderr.decode('utf-8', errors='strict')
            debug_name = name.removesuffix('.log') + '-loader-files.log'
            (self.root/debug_name).write_text(debug)
            self.calls[-1].update(environmentOverrides={'LD_DEBUG': 'files'}, stderrLog=debug_name)
        require(result.returncode == 0, 'Dependency measurement failed: ' + name)
        return (text, debug) if loader_files else text


def elf_needed(text):
    require(not re.search(r'\((?:RPATH|RUNPATH|AUDIT|DEPAUDIT|FILTER|AUXILIARY)\)', text),
            'ELF search paths or injected loader dependencies unsupported')
    return re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', text)


def elf_main_mapping(debug, binary, program):
    # rtld prints a file-labelled link-map block before clearing the main name.
    # Bind its base/dynamic/entry/phdr to this exact executable's program headers.
    pattern = (r'^\s*([0-9]+):\s+file=' + re.escape(str(binary))
        + r' \[0\];\s+generating link map\n'
        r'\s*\1:\s+dynamic: (0x[0-9a-f]+)\s+base: (0x[0-9a-f]+)\s+size: (0x[0-9a-f]+)\n'
        r'\s*\1:\s+entry: (0x[0-9a-f]+)\s+phdr: (0x[0-9a-f]+)\s+phnum:\s+([0-9]+)\s*$')
    maps = re.findall(pattern, debug, re.M)
    require(len(maps) == 1, 'Unnamed ELF mapping lacks unique exact executable diagnostic')
    _, dynamic, base, size, entry, phdr, count = maps[0]
    dynamic, base, size, entry, phdr = [int(x, 16) for x in (dynamic, base, size, entry, phdr)]
    def one(pattern):
        matches = re.findall(pattern, program, re.M)
        require(len(matches) == 1, 'Executable program header binding ambiguous')
        return matches[0]
    dynamic_vaddr = int(one(r'^\s*DYNAMIC\s+0x[0-9a-f]+\s+(0x[0-9a-f]+)\s'), 16)
    phdr_vaddr = int(one(r'^\s*PHDR\s+0x[0-9a-f]+\s+(0x[0-9a-f]+)\s'), 16)
    first_load = re.findall(r'^\s*LOAD\s+(0x[0-9a-f]+)\s+(0x[0-9a-f]+)\s', program, re.M)
    kind = one(r'^Elf file type is (EXEC|DYN) ')
    # a position-independent executable (DYN) is mapped at base, its first
    # LOAD at vaddr 0; a fixed-address one (EXEC, as bun's Linux builds are)
    # has base 0 and its segments at their own addresses (aarch64 bun: the
    # first LOAD at 0x200000), so the same binding holds with base 0
    require(first_load and int(first_load[0][0], 16) == 0
            and (int(first_load[0][1], 16) == 0 if kind == 'DYN' else True),
            'Unnamed main mapping requires the executable LOAD at file offset 0 (and vaddr 0 for a PIE)')
    require((base > 0 if kind == 'DYN' else base == 0) and size > 0 and dynamic == base + dynamic_vaddr
        and entry == base + int(one(r'^Entry point (0x[0-9a-f]+)$'), 16)
        and phdr == base + phdr_vaddr
        and int(count) == int(one(r'^There are ([0-9]+) program headers,')),
        'Unnamed ELF mapping does not bind executable program headers')
    # the unnamed row shows the link map's address: base for a PIE; for a
    # fixed-address executable glibc's l_addr (0), or its first LOAD
    starts = [hex(base)] if kind == 'DYN' else [hex(base), hex(int(first_load[0][1], 16))]
    return {'path': str(binary), 'sha256': sha(binary), 'mapStart': hex(base), 'mapStarts': starts, 'elfType': kind,
        'binding': 'Same --list invocation LD_DEBUG=files exact file, base/dynamic/entry/phdr/phnum',
        'classification': 'measured-main-executable-not-dependency'}


def elf_resolved(text, main_mapping=None):
    result = {}
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    for index, line in enumerate(lines):
        line = line.strip()
        anonymous = re.fullmatch(r'\(0x([0-9a-f]+)\)', line)
        if anonymous:
            require(index == 0 and main_mapping is not None
                and int(anonymous[1], 16) in [int(x, 16) for x in main_mapping.get('mapStarts', [main_mapping['mapStart']])],
                'Unbound or misplaced unnamed ELF mapping')
            continue
        if re.fullmatch(r'linux-vdso\.so\.1 \(0x[0-9a-f]+\)', line):
            continue  # kernel virtual mapping; not a library file
        m = re.fullmatch(r'([^ ]+) => (/[^ ]+) \(0x[0-9a-f]+\)', line)
        if m:
            require(m[1] not in result, 'Duplicate loader resolution')
            result[m[1]] = m[2]
            continue
        m = re.fullmatch(r'(/[^ ]+) \(0x[0-9a-f]+\)', line)
        require(m is not None, 'Unresolved or unexpected ELF loader output')
        require(Path(m[1]).name not in result, 'Duplicate loader resolution')
        result[Path(m[1]).name] = m[1]
    return result


def no_loader_imports(symbols):
    require(not re.search(r'\b_?(?:dlopen|dlmopen|dlsym|dlvsym|__libc_dlopen_mode)(?:@|\b)', symbols),
            'Runtime loading API requires a separately measured dynamic closure')


def loader_imports(symbols, dynamic):
    # Bun imports dlopen/dlsym for bun:ffi; that is allowed only with the
    # declared dynamic loads of ffi_libraries.
    used = sorted(set(re.findall(r'\b(_?(?:dlopen|dlmopen|dlsym|dlvsym|__libc_dlopen_mode))(?:@|\b)', symbols)))
    if dynamic is None:
        no_loader_imports(symbols)
    require(not any(n.lstrip('_') in {'dlmopen', 'dlvsym', 'libc_dlopen_mode'} for n in used),
            'Unexpected runtime loading API')
    return used


def pe(data):
    def unpack(fmt, at):
        require(0 <= at <= len(data)-struct.calcsize(fmt), 'Truncated PE data')
        return struct.unpack_from(fmt, data, at)
    require(data[:2] == b'MZ', 'Expected PE')
    at, = unpack('<I', 0x3c)
    require(data[at:at+4] == b'PE\0\0', 'Invalid PE signature')
    machine, count = unpack('<HH', at+4)
    length, = unpack('<H', at+20)
    opt = at + 24
    require(machine == 0x8664 and length >= 240 and 1 <= count <= 96
            and opt+length+count*40 <= len(data) and unpack('<H', opt)[0] == 0x20b,
            'AMD64 PE32+ required')
    sections = []
    for i in range(count):
        vs, va, size, raw = unpack('<IIII', opt+length+40*i+8)
        sections.append((va, size, raw))
    def off(rva):
        for va, size, raw in sections:
            if va <= rva < va+size:
                return raw+rva-va
        require(rva < unpack('<I', opt+60)[0], 'PE RVA outside file-backed data')
        return rva
    def string(rva):
        start = off(rva)
        end = data.find(b'\0', start)
        require(end >= 0, 'Unterminated PE string')
        return data[start:end].decode('ascii')
    result = {'machine': '0x8664', 'imports': [], 'delayImports': []}
    for index, width, fmt, field, key in ((1, 20, '<IIIII', 3, 'imports'), (13, 32, '<IIIIIIII', 1, 'delayImports')):
        rva, size = unpack('<II', opt+112+index*8)
        if not rva:
            continue
        for n in range(min(size // width, 4096)):
            row = unpack(fmt, off(rva)+width*n)
            if not any(row):
                break
            name = row[field]
            if index == 13 and not row[0] & 1:
                name -= unpack('<Q', opt+24)[0]
            dll = string(name)
            require(re.fullmatch(r'[A-Za-z0-9_.-]+\.dll', dll, re.I), 'Invalid imported DLL name')
            result[key].append(dll)
        else:
            raise ValueError('Unterminated PE import directory')
    return result


def windows_system(name):
    return name.lower().removesuffix('.dll') in SYSTEM_DLLS or bool(APISET.fullmatch(name.lower()))


def windows_measure(binary, target, dynamic, system32=None):
    # Static PE imports (normal and delay) of the service, every one an OS DLL.
    service = pe(Path(binary).read_bytes())
    require(service['imports'], 'PE imports nothing')
    system32 = Path(system32 or os.path.join(os.environ.get('SystemRoot', r'C:\Windows'), 'System32'))
    nodes, edges, seen = [], [], set()
    for mode, kind in (('imports', 'normal'), ('delayImports', 'delay')):
        for dep in service[mode]:
            require(windows_system(dep), 'Non-system DLL import requires supplied runtime qualification: ' + dep)
            edges.append({'from': 'service', 'name': dep, 'kind': kind, 'resolvedSystemPath':
                          'apiset:' + dep.lower() if APISET.fullmatch(dep.lower()) else str(system32/dep.lower())})
            if dep.lower() in seen:
                continue
            seen.add(dep.lower())
            if APISET.fullmatch(dep.lower()):
                nodes.append({'name': dep.lower(), 'classification': 'approved-system', 'redistributed': False,
                              'architecture': target, 'apiSet': True})
                continue
            path = system32/dep.lower()
            require(path.is_file(), 'Imported system DLL is not on this host: ' + dep)
            pe(path.read_bytes())
            nodes.append({'name': dep.lower(), 'path': str(path), 'sha256': sha(path), 'bytes': path.stat().st_size,
                          'architecture': target, 'classification': 'approved-system', 'redistributed': False})
    return {'platform': 'Windows', 'release': platform.release(), 'windowsVersion': platform.version(),
        'peImports': service, 'systemLibraries': nodes, 'dependencyGraph': edges, 'runtimeLoaderImports': [],
        'dynamicLoads': dynamic,
        'sourceDisposition': 'Microsoft Windows system DLLs; no system binary is copied or redistributed.'}


def linux_measure(binary, target, capture, dynamic=None):
    loader_name = 'ld-linux-x86-64.so.2' if target == 'linux-x64' else 'ld-linux-aarch64.so.1'
    program = capture(['/usr/bin/readelf', '--program-headers', '--wide', binary])
    interpreters = re.findall(r'Requesting program interpreter: ([^\]]+)\]', program)
    require(len(interpreters) == 1 and Path(interpreters[0]).name == loader_name, 'Unexpected ELF interpreter')
    loader = Path(interpreters[0])
    require(loader.is_absolute() and str(loader.resolve()).startswith(('/usr/lib/', '/lib/')), 'Foreign ELF interpreter')
    listed, debug = capture.elf_list(loader, binary)
    main_mapping = elf_main_mapping(debug, binary, program) if re.search(r'^\s*\(0x[0-9a-f]+\)\s*$', listed, re.M) else None
    resolved = elf_resolved(listed, main_mapping)
    loader_version = capture([loader, '--version'])
    approved = {'libc.so.6', 'libm.so.6', 'libpthread.so.0', 'libdl.so.2', 'librt.so.1', loader_name}
    require(resolved and set(resolved) <= approved and loader_name in resolved, 'Non-system dependency requires supplied runtime qualification')
    for name, path in resolved.items():
        require(Path(path).name == name and str(Path(path).resolve()).startswith(('/usr/lib/', '/lib/')),
                'Dependency resolved outside approved system library directories')
    nodes, edges = [], []
    loads = []
    queue = [('service', binary)] + [(name, Path(path)) for name, path in sorted(resolved.items())]
    for name, path in queue:
        needed = elf_needed(capture(['/usr/bin/readelf', '--dynamic', '--wide', path]))
        require(all(n in resolved for n in needed), 'Mandatory transitive ELF dependency unresolved')
        if name == 'service':
            loads = loader_imports(capture(['/usr/bin/readelf', '--dyn-syms', '--wide', path]), dynamic)
        else:
            with path.open('rb') as stream:
                header = stream.read(20)
            require(header[:6] == b'\x7fELF\x02\x01' and len(header) == 20
                and struct.unpack_from('<H', header, 18)[0] == (62 if target == 'linux-x64' else 183),
                'System library architecture mismatch')
            nodes.append({'name': name, 'path': str(path), 'realPath': str(path.resolve()),
                'sha256': sha(path), 'bytes': path.stat().st_size, 'architecture': target,
                'classification': 'approved-system', 'redistributed': False})
        edges.extend({'from': name, 'name': dep, 'resolvedSystemPath': resolved[dep]} for dep in needed)
    edges.append({'from': 'service', 'name': loader_name, 'resolvedSystemPath': str(loader), 'interpreter': True})
    for item in dynamic or []:
        require(item['name'] in resolved, 'Dynamically loaded library is not the measured system one')
        edges.append({'from': 'service', 'name': item['name'], 'kind': 'dynamic', 'resolvedSystemPath': resolved[item['name']]})
    os_release = Path('/etc/os-release').read_text()
    return {'platform': 'Linux', 'release': platform.release(), 'osRelease': os_release, 'loaderVersion': loader_version,
        'mainExecutableMapping': main_mapping,
        'systemLibraries': nodes, 'dependencyGraph': edges, 'runtimeLoaderImports': loads,
        'dynamicLoads': dynamic or [],
        'kernelVirtualDso': 'OS-owned linux-vdso.so.1; no redistributable file',
        'sourceDisposition': 'Host distribution C-library package; system libraries are not copied or redistributed.'}


def macho_dependencies(text):
    lines = text.splitlines()
    require(lines and lines[0].endswith(':'), 'Invalid otool dependency report')
    dependencies = []
    for line in lines[1:]:
        m = re.fullmatch(r'\s+(/[^ ]+) \(compatibility version ([0-9.]+), current version ([0-9.]+)(?:, [^)]*)?\)', line)
        require(m is not None and m[1] in APPLE_SYSTEM, 'Mach-O dependency outside approved Apple system ABI')
        require(m[1] not in [d['name'] for d in dependencies], 'Duplicate Mach-O dependency')
        dependencies.append({'name': m[1], 'compatibilityVersion': m[2], 'currentVersion': m[3]})
    require('/usr/lib/libSystem.B.dylib' in [d['name'] for d in dependencies], 'Actual libSystem dependency required')
    return dependencies


def darwin_measure(binary, target, capture, dynamic=None):
    deps = macho_dependencies(capture(['/usr/bin/otool', '-L', binary]))
    commands = capture(['/usr/bin/otool', '-l', binary])
    require(not re.search(r'\bcmd LC_(?:RPATH|LOAD_WEAK_DYLIB|REEXPORT_DYLIB|LOAD_UPWARD_DYLIB|LAZY_LOAD_DYLIB|DYLD_ENVIRONMENT)\b', commands),
            'Unqualified Mach-O search path or secondary dependency command')
    require(re.search(r'name /usr/lib/dyld\b', commands), 'Expected Apple dyld interpreter')
    loads = loader_imports(capture(['/usr/bin/nm', '-u', binary]), dynamic)
    version = capture(['/usr/bin/sw_vers'])
    # Apple's shared-cache system ABI need not exist as a standalone disk file.
    # Resolve each through this host's real dyld, then confirm the loaded image.
    system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
    for d in deps:
        ctypes.CDLL(d['name'])
    system._dyld_image_count.restype = ctypes.c_uint32
    system._dyld_get_image_name.argtypes = [ctypes.c_uint32]
    system._dyld_get_image_name.restype = ctypes.c_char_p
    images = [system._dyld_get_image_name(i).decode() for i in range(system._dyld_image_count())]
    system._dyld_get_image_header.argtypes = [ctypes.c_uint32]
    system._dyld_get_image_header.restype = ctypes.c_void_p
    cpu = 0x1000007 if target == 'darwin-x64' else 0x100000c
    for d in deps:
        require(d['name'] in images, 'Host dyld did not load required system ABI')
        header = ctypes.string_at(system._dyld_get_image_header(images.index(d['name'])), 12)
        require(struct.unpack_from('<II', header) == (0xfeedfacf, cpu), 'Loaded system image is not the target Mach-O')
    edges = [{'from': 'service', 'name': d['name'], 'resolvedSystemPath': d['name']} for d in deps]
    # libSystem.dylib is the umbrella's unversioned name in the same shared cache
    edges += [{'from': 'service', 'name': item['name'], 'kind': 'dynamic',
               'resolvedSystemPath': '/usr/lib/libSystem.B.dylib'} for item in dynamic or []]
    return {'platform': 'Darwin', 'release': platform.release(), 'swVers': version,
        'systemLibraries': [{**d, 'classification': 'approved-system', 'redistributed': False,
            'architecture': target, 'dyldImageObserved': True} for d in deps],
        'dependencyGraph': edges, 'dynamicLoads': dynamic or [],
        'runtimeLoaderImports': loads, 'systemAbiBoundary': 'Apple system libraries and their OS-owned shared-cache closure',
        'sourceDisposition': 'Apple OS-provided libSystem/dyld; no system binary is copied. https://opensource.apple.com/'}


# Windows: the hub needs sh/bash and coreutils for its children (Proc.run:
# git, gh, curl, helpers) and terminals. 0.12.1 ships 0.12.0's portable
# Cygwin runtime and its two Bun helpers unchanged: the exact bytes its
# RUNTIME.json lists, with that release's measured static-import closure.
WINDOWS_RUNTIME_SHA256 = 'e455abb52858638bd5bb341c2a9c7395e3efd9bf8454867038e8844a17f9adc3'
WINDOWS_CLOSURE_SHA256 = 'eec3914d9425a91c8c23d08756935a7e455b035c3daef3dc8a7425cc3c4ae3b9'


def windows_bundle(src, runtime):
    """Copy the pinned bundle in SRC (RUNTIME.json, CLOSURE.json, the members)
    into the service runtime folder; answer its inventory, packages and the
    closure edges of its own binaries (the old C service's edges are dropped)."""
    src = Path(src)
    require(sha(src/'RUNTIME.json') == WINDOWS_RUNTIME_SHA256, 'Windows runtime bundle is not the pinned one')
    require(sha(src/'CLOSURE.json') == WINDOWS_CLOSURE_SHA256, 'Windows runtime closure is not the pinned one')
    manifest = json.loads((src/'RUNTIME.json').read_text())
    closure = json.loads((src/'CLOSURE.json').read_text())
    require(manifest.get('target') == 'windows-x64' and closure.get('target') == 'windows-x64'
            and closure.get('status') == 'PASS' and closure.get('unresolvedMandatory') == [], 'Pinned Windows runtime is not a passing windows-x64 closure')
    members = manifest['members']
    # its licences sit beside the service's own (licenses/{bun,bend}), so they
    # go under runtime-licenses/; every path that names one follows
    place = lambda name: 'runtime-licenses/' + name[len('licenses/'):] if name.startswith('licenses/') else name
    for name, m in sorted(members.items()):
        require(m.get('type', 'file') == 'file' and not name.startswith(('/', '../')) and '/../' not in name, 'Unsafe bundle member: ' + name)
        f = src/name
        require(f.is_file() and not f.is_symlink() and f.stat().st_size == m['bytes'] and sha(f) == m['sha256'], 'Bundle member differs: ' + name)
        dest = runtime/place(name)
        require(not dest.exists(), 'Bundle member collides: ' + name)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(f, dest)
    edges = [e for e in closure['edges'] if e.get('from') in members]
    for e in edges:
        require(e.get('kind') in ('normal', 'delay', 'dynamic') and e.get('name'), 'Unusable bundle closure edge')
        if not e.get('system'):
            require(e.get('resolved') in members, 'Bundle edge resolves outside the bundle: ' + str(e.get('resolved')))
    inventory = [{**i, 'path': place(i['path'])} for i in manifest['inventory']]
    packages = [{**p, 'licenses': [{**l, 'path': place(l['path'])} for l in p.get('licenses', [])]} for p in manifest['packages']]
    require(not any(e.get('from', '').startswith('licenses/') or str(e.get('resolved') or '').startswith('licenses/') for e in edges), 'A closure edge names a licence file')
    return inventory, packages, edges


def prepare(args):
    producer = args.producer.resolve(strict=True)
    require(sha(producer) == args.producer_sha256, 'Component producer hash mismatch')
    spec = importlib.util.spec_from_file_location('service_component_producer', producer)
    service = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(service)
    service.ROOT = args.source_root.resolve(strict=True)
    source, out = args.build.resolve(strict=True), args.out
    raw = source/'BUILD.json'
    build = service.read(raw)
    target = build.get('target')
    require(target == target_host() and target in service.TARGETS, 'Matching actual target host required')
    require(not any(k.startswith(('LD_', 'DYLD_')) for k in os.environ), 'Ambient loader overrides refused')
    release = {'version': service.VERSION, 'tag': 'v'+service.VERSION, 'channel': 'release',
        'commit': build['source_commit'], 'tree': build['source_tree']}
    service.frozen(release)
    require(build.get('status') == 'VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD'
        and build.get('compileInputs') == service.compile_inputs()
        and build.get('serviceRuntime') == 'bun', 'Original configured JS service BUILD required')
    name = service.service_name(target)
    binary = source/name
    require(sha(binary) == build['binary_sha256'], 'Service binary changed')
    service.architecture(binary, target)
    files = service.git('ls-files').splitlines()
    require(service.read(source/'SOURCE.json')['members'] == {
        n: sha(service.ROOT/n) for n in files}, 'Service source manifest differs from frozen checkout')
    dynamic = ffi_libraries(service.ROOT, files, target)
    service.new_output(out)
    out.mkdir(parents=True)
    runtime = out/'runtime'
    evidence = runtime/'runtime-evidence'
    evidence.mkdir(parents=True)
    capture = Capture(evidence)
    measure = {'linux': linux_measure, 'darwin': darwin_measure}.get(family(target))
    measurement = measure(binary, target, capture, dynamic) if measure else windows_measure(binary, target, dynamic)
    for edge in measurement['dependencyGraph']:
        if edge['from'] == 'service':
            edge['from'] = name
    measurement.update({'schema': 1, 'target': target, 'sourceCommit': release['commit'], 'sourceTree': release['tree'],
        'binarySha256': sha(binary), 'serviceRuntime': 'bun', 'operations': capture.calls,
        'scope': 'Measured service dynamic ABI closure (static imports and declared bun:ffi loads) only; '
                 'external CLI tools and installed UI/workflows are not qualified.'})
    write(evidence/'system-dependencies.json', measurement)
    shutil.copyfile(Path(__file__), evidence/'SERVICE-RUNTIME-PRODUCER.py')
    notice = runtime/'runtime-notices'
    notice.mkdir()
    shutil.copyfile(source/'LICENSE.Backplane', notice/'LICENSE.Backplane')
    (notice/'SYSTEM-ABI-NOTICE.txt').write_text('System libraries are host supplied and NOT redistributed.\n'
        + measurement['sourceDisposition'] + '\nSee runtime-evidence/system-dependencies.json for actual paths, '
        'versions/hashes where files exist and measurement commands. Apple shared-cache images are recorded as such; '
        'no fictitious file hashes are assigned.\nNo library, font, SDK or compiler runtime was silently bundled.\n')
    packages = [{'name': 'Backplane runtime measurement and declaration', 'version': service.VERSION,
        'sourceDisposition': 'https://github.com/i2cjak/Backplane/tree/'+release['commit']+'; actual separately pinned measurement producer retained in runtime-evidence.',
        'licenses': [{'path': 'runtime-notices/LICENSE.Backplane', 'sha256': sha(notice/'LICENSE.Backplane')},
                     {'path': 'runtime-notices/SYSTEM-ABI-NOTICE.txt', 'sha256': sha(notice/'SYSTEM-ABI-NOTICE.txt')}]}]
    inventory = [{'path': f.relative_to(runtime).as_posix(), 'package': packages[0]['name'], 'version': service.VERSION,
         'architecture': target, 'sha256': sha(f), 'bytes': f.stat().st_size} for f in sorted(evidence.iterdir())]
    bundled = []
    if target == 'windows-x64':
        require(args.windows_runtime is not None, 'windows-x64 needs --windows-runtime: the hub has no shell without it')
        extra, extra_packages, bundled = windows_bundle(args.windows_runtime, runtime)
        inventory += extra
        packages += extra_packages
    write(runtime/'RUNTIME.json', {'schema': 1, 'target': target, 'inventory': inventory, 'packages': packages,
        'suppliedLibraries': [], 'systemDependencies': measurement['systemLibraries'],
        **({'bundledRuntime': {'runtimeManifestSha256': WINDOWS_RUNTIME_SHA256, 'closureSha256': WINDOWS_CLOSURE_SHA256,
             'source': '0.12.0 portable Cygwin runtime and Bun helpers, unchanged'}} if bundled else {})})
    # Edges are rooted in the supplied service; system dependencies remain OS-owned
    # leaves. The full measured Linux transitive graph is retained in system.json.
    write(runtime/'CLOSURE.json', {'schema': 1, 'target': target, 'measured': True, 'status': 'PASS',
        'unresolvedMandatory': [], 'edges': [{'from': name, 'kind': d.get('kind', 'normal'), 'name': d['name'],
          'system': True, 'classification': 'approved-system', 'resolvedSystemPath': d['resolvedSystemPath']}
          for d in measurement['dependencyGraph'] if d['from'] == name] + bundled,
        'measurement': 'runtime-evidence/system-dependencies.json', 'runtimeAcceptance': 'PENDING'})
    write(out/'RUNTIME-QUALIFICATION.json', {'schema': 1, 'kind': 'backplane-service-runtime-qualification',
        'status': 'PASS', 'target': target, 'sourceCommit': release['commit'], 'sourceTree': release['tree'],
        'binarySha256': sha(binary), 'runtimeManifestSha256': sha(runtime/'RUNTIME.json'),
        'closureSha256': sha(runtime/'CLOSURE.json'), 'members': service.inventory(runtime),
        'scope': measurement['scope'], 'componentProducer': service.reference(producer)})
    request = {'schema': 1, 'kind': 'backplane-service-component-inputs', 'target': target, 'release': release,
        'buildDirectory': str(source), 'buildReceipt': service.reference(raw), 'runtimeDirectory': str(runtime),
        'runtimeQualification': service.reference(out/'RUNTIME-QUALIFICATION.json')}
    write(out/'COMPONENT-REQUEST.json', request)
    result = service.component(request, out, out/'component')
    require(sha(binary) == build['binary_sha256'] and sha(raw) == request['buildReceipt']['sha256'], 'Build changed during measurement')
    service.frozen(release)
    write(out/'RESULT.json', {**result, 'runtimeAcceptance': 'PENDING', 'published': False,
        'measurementProducer': service.reference(Path(__file__)), 'componentProducer': service.reference(producer)})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('source-root', 'build', 'producer', 'out'):
        parser.add_argument('--'+name, type=Path, required=True)
    parser.add_argument('--producer-sha256', required=True)
    parser.add_argument('--windows-runtime', type=Path, help='windows-x64: the pinned bundle folder (RUNTIME.json, CLOSURE.json, members)')
    args = parser.parse_args()
    require(args.out.is_absolute(), 'New absolute output directory required')
    print(json.dumps(prepare(args), indent=2))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
        print('Service runtime preparation: '+str(error), file=sys.stderr)
        sys.exit(1)
