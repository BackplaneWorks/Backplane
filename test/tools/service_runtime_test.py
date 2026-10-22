#!/usr/bin/env python3
"""Offline parser/closure failures; no configured artifact or runtime acceptance."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('runtime', ROOT/'scripts/service-runtime.py')
R = importlib.util.module_from_spec(spec)
spec.loader.exec_module(R)


def pe_image(imports=(), delay=(), machine=0x8664):
    # A minimal PE32+: one section at RVA 0x1000 holding the import tables and names.
    data = bytearray(0x400)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 0x40)
    data[0x40:0x44] = b'PE\0\0'
    struct.pack_into('<HH', data, 0x44, machine, 1)
    struct.pack_into('<H', data, 0x40+20, 240)
    opt = 0x40 + 24
    struct.pack_into('<H', data, opt, 0x20b)
    struct.pack_into('<Q', data, opt+24, 0x140000000)
    struct.pack_into('<I', data, opt+60, 0x200)
    struct.pack_into('<8sIIII', data, opt+240, b'.idata', 0x200, 0x1000, 0x200, 0x200)
    names = 0x1100
    def place(rows, width, at, field, attrs):
        nonlocal names
        for i, dll in enumerate(rows):
            struct.pack_into('<I', data, 0x200 + at - 0x1000 + width*i + 4*field, names)
            if attrs:
                struct.pack_into('<I', data, 0x200 + at - 0x1000 + width*i, 1)
            raw = dll.encode() + b'\0'
            data[0x200 + names - 0x1000:0x200 + names - 0x1000 + len(raw)] = raw
            names += len(raw)
    if imports:
        place(imports, 20, 0x1000, 3, False)
        struct.pack_into('<II', data, opt+112+8, 0x1000, 20*(len(imports)+1))
    if delay:
        place(delay, 32, 0x1080, 1, True)
        struct.pack_into('<II', data, opt+112+13*8, 0x1080, 32*(len(delay)+1))
    return bytes(data)


class RuntimeTests(unittest.TestCase):
    PROGRAM = ('Elf file type is DYN (Position-Independent Executable file)\n'
        'Entry point 0x18b22c0\nThere are 9 program headers, starting at offset 64\n'
        ' PHDR 0x000040 0x0000000000000040 0x40\n'
        ' LOAD 0x000000 0x0000000000000000 0x0\n'
        ' DYNAMIC 0x587fda0 0x000000000587fda0 0x587fda0\n')
    ARM_LIST = ('\t (0x0000ff9b5c8e0000)\n'
        '\tlinux-vdso.so.1 (0x0000ff9b622ac000)\n'
        '\tlibm.so.6 => /lib/aarch64-linux-gnu/libm.so.6 (0x0000ff9b5c830000)\n'
        '\tlibc.so.6 => /lib/aarch64-linux-gnu/libc.so.6 (0x0000ff9b5c670000)\n'
        '\t/lib/ld-linux-aarch64.so.1 (0x0000ff9b62270000)\n')

    def debug(self, binary):
        # Synthetic glibc diagnostic with preserved ARM program-header offsets.
        base = 0xff9b5c8e0000
        return (f' 42: file={binary} [0];  generating link map\n'
            f' 42:   dynamic: 0x{base+0x587fda0:016x}  base: 0x{base:016x}   size: 0x0000000005980000\n'
            f' 42:     entry: 0x{base+0x18b22c0:016x}  phdr: 0x{base+0x40:016x}  phnum:   9\n\n')

    def test_actual_arm_row_requires_same_invocation_main_binding(self):
        with self.assertRaises(ValueError):
            R.elf_resolved(self.ARM_LIST)
        with tempfile.TemporaryDirectory() as d:
            binary = Path(d)/'backplane-serve'
            binary.write_bytes(b'parser-model-only')
            bound = R.elf_main_mapping(self.debug(binary), binary, self.PROGRAM)
            got = R.elf_resolved(self.ARM_LIST, bound)
            self.assertEqual(set(got), {'libm.so.6', 'libc.so.6', 'ld-linux-aarch64.so.1'})
            self.assertEqual(bound['sha256'], R.sha(binary))

    # bun's linux-arm64 build: a fixed-address executable (EXEC), its first
    # LOAD at offset 0 and vaddr 0x200000, base 0 in the link map
    EXEC_PROGRAM = ('Elf file type is EXEC (Executable file)\n'
        'Entry point 0x2a68700\nThere are 9 program headers, starting at offset 64\n'
        ' PHDR 0x000040 0x0000000000200040 0x0000000000200040 0x0001f8 0x0001f8 R 0x8\n'
        ' LOAD 0x000000 0x0000000000200000 0x0000000000200000 0x26df620 0x26df620 R 0x10000\n'
        ' LOAD 0x26e8700 0x0000000002a68700 0x0000000002a68700 0x3931300 0x3931300 R E 0x10000\n'
        ' DYNAMIC 0x60a8000 0x0000000006438000 0x0000000006438000 0x0001f0 0x0001f0 RW 0x8\n')

    def exec_debug(self, binary, base=0):
        return (f' 42: file={binary} [0];  generating link map\n'
            f' 42:   dynamic: 0x{base+0x6438000:016x}  base: 0x{base:016x}   size: 0x0000000006320000\n'
            f' 42:     entry: 0x{base+0x2a68700:016x}  phdr: 0x{base+0x200040:016x}  phnum:   9\n\n')

    def test_fixed_address_executable_binds_with_base_zero(self):
        with tempfile.TemporaryDirectory() as d:
            binary = Path(d)/'backplane-serve'
            binary.write_bytes(b'parser-model-only')
            bound = R.elf_main_mapping(self.exec_debug(binary), binary, self.EXEC_PROGRAM)
            self.assertEqual(bound['elfType'], 'EXEC')
            for row in ('0x0000000000000000', '0x0000000000200000'):
                listed = self.ARM_LIST.replace('0x0000ff9b5c8e0000', row)
                self.assertIn('libc.so.6', R.elf_resolved(listed, bound))
            with self.assertRaises(ValueError):
                R.elf_resolved(self.ARM_LIST.replace('0x0000ff9b5c8e0000', '0x0000000000300000'), bound)
            # an EXEC mapped anywhere but its own addresses, or a PIE read as one, fails
            with self.assertRaises(ValueError):
                R.elf_main_mapping(self.exec_debug(binary, 0x1000), binary, self.EXEC_PROGRAM)
            with self.assertRaises(ValueError):
                R.elf_main_mapping(self.exec_debug(binary), binary, self.EXEC_PROGRAM.replace('EXEC (Executable file)', 'DYN (Position-Independent Executable file)'))
            with self.assertRaises(ValueError):
                R.elf_main_mapping(self.exec_debug(binary), binary, self.EXEC_PROGRAM.replace('Elf file type is EXEC (Executable file)\n', ''))

    def test_main_binding_requires_exact_file_and_all_headers(self):
        with tempfile.TemporaryDirectory() as d:
            binary = Path(d)/'backplane-serve'
            binary.write_bytes(b'parser-model-only')
            debug = self.debug(binary)
            cases = [debug.replace(str(binary), '/foreign'), debug+debug,
                debug.replace('phnum:   9', 'phnum:   8'),
                debug.replace('0x0000ff9b6215fda0', '0x0000ff9b6215fda1'),
                debug.replace('0x0000ff9b5e1922c0', '0x0000ff9b5e1922c1'),
                debug.replace('0x0000ff9b5c8e0040', '0x0000ff9b5c8e0041'),
                debug.replace(' 42:     entry:', ' 43:     entry:'), '']
            for text in cases:
                with self.subTest(text=text), self.assertRaises(ValueError):
                    R.elf_main_mapping(text, binary, self.PROGRAM)

    def test_bound_main_does_not_allow_other_anonymous_or_bad_dependencies(self):
        bound = {'mapStart': '0xff9b5c8e0000'}
        for text in [self.ARM_LIST.replace('5c8e0000', '5c8e0001'),
            self.ARM_LIST+self.ARM_LIST.splitlines()[0]+'\n',
            'linux-vdso.so.1 (0x1)\n'+self.ARM_LIST,
            self.ARM_LIST+'libx.so => not found\n', self.ARM_LIST+'unexpected\n',
            self.ARM_LIST+' /foreign/ld-linux-aarch64.so.1 (0x2)\n']:
            with self.subTest(text=text), self.assertRaises(ValueError):
                R.elf_resolved(text, bound)

    def test_loader_diagnostic_is_fixed_and_retains_separate_streams(self):
        with tempfile.TemporaryDirectory() as d:
            c = R.Capture(Path(d))
            result = R.subprocess.CompletedProcess([], 0, b'list', b'diagnostic')
            with patch.object(R.subprocess, 'run', return_value=result) as run:
                self.assertEqual(c.elf_list(Path('/usr/bin/true'), Path('/exact/binary')), ('list', 'diagnostic'))
                self.assertEqual(run.call_args.args[0], ['/usr/bin/true', '--list', '/exact/binary'])
                self.assertEqual({k:v for k,v in run.call_args.kwargs['env'].items() if k.startswith('LD_')}, {'LD_DEBUG':'files'})
            self.assertEqual((Path(d)/c.calls[0]['stderrLog']).read_text(), 'diagnostic')

    def test_native_host_tuple_is_exact(self):
        for system, machine, target in [('Linux', 'x86_64', 'linux-x64'), ('Linux', 'aarch64', 'linux-arm64'),
                                        ('Darwin', 'arm64', 'darwin-arm64'), ('Darwin', 'x86_64', 'darwin-x64'),
                                        ('Windows', 'AMD64', 'windows-x64'), ('Windows', 'ARM64', None),
                                        ('Linux', 'armv7l', None)]:
            with patch.object(R.platform, 'system', return_value=system), patch.object(R.platform, 'machine', return_value=machine):
                self.assertEqual(R.target_host(), target)

    def test_elf_uses_actual_needed_not_symbol_guess(self):
        self.assertEqual(R.elf_needed(' 0x1 (NEEDED) Shared library: [libc.so.6]\n'), ['libc.so.6'])
        for tag in ['RPATH', 'RUNPATH', 'AUDIT', 'DEPAUDIT', 'FILTER', 'AUXILIARY']:
            with self.assertRaises(ValueError):
                R.elf_needed(' ('+tag+') [/unapproved]')

    def test_loader_reports_actual_resolution_and_interpreter(self):
        value = R.elf_resolved(' linux-vdso.so.1 (0x123)\n libc.so.6 => /lib/aarch64-linux-gnu/libc.so.6 (0x456)\n /lib/ld-linux-aarch64.so.1 (0x789)\n')
        self.assertEqual(value, {'libc.so.6': '/lib/aarch64-linux-gnu/libc.so.6', 'ld-linux-aarch64.so.1': '/lib/ld-linux-aarch64.so.1'})

    def test_unresolved_and_unrecognized_loader_output_refused(self):
        for text in ['libc.so.6 => not found', 'unrecognized output', 'libc.so.6 => /lib/libc.so.6 (0x1)\nlibc.so.6 => /tmp/libc.so.6 (0x2)']:
            with self.assertRaises(ValueError):
                R.elf_resolved(text)

    def test_runtime_loader_api_refused_on_both_formats(self):
        for symbol in ['dlopen@GLIBC_2.34', '_dlopen', 'dlmopen', 'dlsym@GLIBC_2.2', '_dlsym', 'dlvsym', '__libc_dlopen_mode']:
            with self.assertRaises(ValueError):
                R.no_loader_imports(' UND '+symbol)
        R.no_loader_imports(' UND socket@GLIBC_2.2.5\n UND pthread_create@GLIBC_2.34')

    def test_macho_system_umbrella_version_measured(self):
        value = R.macho_dependencies('/service:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1345.120.2)\n')
        self.assertEqual(value[0]['currentVersion'], '1345.120.2')

    def test_macho_bun_system_libraries_approved(self):
        # what Bun 1.x's darwin runtime links (otool -L)
        text = '/service:\n' + ''.join('\t' + n + ' (compatibility version 1.0.0, current version 1.0.0)\n' for n in [
            '/usr/lib/libicucore.A.dylib', '/usr/lib/libresolv.9.dylib', '/usr/lib/libc++.1.dylib', '/usr/lib/libSystem.B.dylib'])
        self.assertEqual(len(R.macho_dependencies(text)), 4)
        with self.assertRaises(ValueError):
            R.macho_dependencies(text.replace('\t/usr/lib/libSystem.B.dylib', '\t/usr/lib/libz.1.dylib'))
        with self.assertRaises(ValueError):
            R.macho_dependencies(text + '\t/usr/lib/libc++.1.dylib (compatibility version 1.0.0, current version 1.0.0)\n')

    def test_bun_loader_imports_need_declared_dynamic_loads(self):
        symbols = ' UND dlopen@GLIBC_2.2.5\n UND dlsym@GLIBC_2.2.5\n'
        with self.assertRaises(ValueError):
            R.loader_imports(symbols, None)
        self.assertEqual(R.loader_imports(symbols, [{'name': 'libc.so.6'}]), ['dlopen', 'dlsym'])
        with self.assertRaises(ValueError):
            R.loader_imports(' UND dlmopen@GLIBC_2.3.4\n', [{'name': 'libc.so.6'}])

    def bundle(self, root):
        # a two-member synthetic Windows runtime bundle: bash.exe importing
        # cygwin1.dll (in the bundle) and KERNEL32.dll (system)
        (root/'runtime/bin').mkdir(parents=True)
        (root/'runtime/bin/bash.exe').write_bytes(b'bash')
        (root/'runtime/bin/cygwin1.dll').write_bytes(b'cyg')
        (root/'licenses').mkdir()
        (root/'licenses/COPYING').write_text('GPL')
        members = {n: {'type': 'file', 'sha256': R.sha(root/n), 'bytes': (root/n).stat().st_size}
                   for n in ('runtime/bin/bash.exe', 'runtime/bin/cygwin1.dll', 'licenses/COPYING')}
        R.write(root/'RUNTIME.json', {'target': 'windows-x64', 'members': members,
            'inventory': [{'path': n, 'package': 'bash', 'version': '5', 'architecture': 'x64', **m} for n, m in members.items()],
            'packages': [{'name': 'bash', 'version': '5', 'sourceDisposition': 'fixture', 'licenses': [{'path': 'licenses/COPYING', 'sha256': members['licenses/COPYING']['sha256']}]}]})
        R.write(root/'CLOSURE.json', {'target': 'windows-x64', 'status': 'PASS', 'unresolvedMandatory': [], 'edges': [
            {'from': 'runtime/bin/bash.exe', 'kind': 'normal', 'name': 'cygwin1.dll', 'resolved': 'runtime/bin/cygwin1.dll'},
            {'from': 'runtime/bin/bash.exe', 'kind': 'normal', 'name': 'KERNEL32.dll', 'system': True, 'classification': 'approved-system'},
            {'from': 'backplane-serve.exe', 'kind': 'normal', 'name': 'cygwin1.dll', 'resolved': 'runtime/bin/cygwin1.dll'}]})

    def test_windows_bundle_copies_the_pinned_runtime_and_its_own_edges(self):
        with tempfile.TemporaryDirectory() as d:
            src, rt = Path(d)/'bundle', Path(d)/'runtime'
            src.mkdir(); rt.mkdir(); self.bundle(src)
            with patch.object(R, 'WINDOWS_RUNTIME_SHA256', R.sha(src/'RUNTIME.json')), patch.object(R, 'WINDOWS_CLOSURE_SHA256', R.sha(src/'CLOSURE.json')):
                inv, packages, edges = R.windows_bundle(src, rt)
            self.assertEqual(len(inv), 3); self.assertEqual(packages[0]['name'], 'bash')
            self.assertEqual((rt/'runtime/bin/bash.exe').read_bytes(), b'bash')
            # the old C service's edge is not carried over
            self.assertEqual(sorted(e['name'] for e in edges), ['KERNEL32.dll', 'cygwin1.dll'])
            self.assertTrue(all(e['from'] == 'runtime/bin/bash.exe' for e in edges))

    def test_windows_bundle_refuses_another_bundle_or_changed_bytes(self):
        with tempfile.TemporaryDirectory() as d:
            src = Path(d)/'bundle'; src.mkdir(); self.bundle(src)
            with self.assertRaises(ValueError):
                R.windows_bundle(src, Path(d)/'rt1')  # the real pins: not this bundle
            pins = (R.sha(src/'RUNTIME.json'), R.sha(src/'CLOSURE.json'))
            (src/'runtime/bin/bash.exe').write_bytes(b'tampered')
            (Path(d)/'rt2').mkdir()
            with patch.object(R, 'WINDOWS_RUNTIME_SHA256', pins[0]), patch.object(R, 'WINDOWS_CLOSURE_SHA256', pins[1]):
                with self.assertRaises(ValueError):
                    R.windows_bundle(src, Path(d)/'rt2')

    def test_ffi_libraries_named_anywhere_in_the_file(self):
        # an effect picks its library into a variable: the literals in the
        # file declare it, and kernel32.dll is the Windows system library
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); (root / 'src').mkdir()
            (root / 'src/e.js').write_text(
                'const lib = mac ? "/usr/lib/libSystem.B.dylib" : "libc.so.6";\n'
                'const c = ffi.dlopen(lib, { flock: {} });\n'
                'const k = ffi.dlopen("kernel32.dll", { CreateFileW: {} });\n')
            files = ['src/e.js']
            self.assertEqual([x['name'] for x in R.ffi_libraries(root, files, 'linux-x64')], ['libc.so.6'])
            self.assertIn('/usr/lib/libSystem.B.dylib', [x['name'] for x in R.ffi_libraries(root, files, 'darwin-arm64')])
            self.assertEqual([x['name'] for x in R.ffi_libraries(root, files, 'windows-x64')], ['kernel32.dll'])
            (root / 'src/f.js').write_text('const n = pick(); ffi.dlopen(n, { a: {} });\n')
            with self.assertRaises(ValueError):
                R.ffi_libraries(root, ['src/f.js'], 'linux-x64')
            (root / 'src/g.js').write_text('ffi.dlopen("user32.dll", { a: {} });\n')
            with self.assertRaises(ValueError):
                R.ffi_libraries(root, ['src/g.js'], 'windows-x64')

    def test_ffi_libraries_from_sources(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root/'src/server/effects').mkdir(parents=True)
            (root/'src/server/effects/a.js').write_text(
                'const l = ffi.dlopen(mac ? "libSystem.dylib" : "libc.so.6", {\n x: {} });\n'
                'b = dlopen(process.platform === "darwin" ? "/usr/lib/libSystem.B.dylib" : "libc.so.6", { y: {} });\n')
            files = ['src/server/effects/a.js', 'README']
            self.assertEqual([x['name'] for x in R.ffi_libraries(root, files, 'linux-arm64')], ['libc.so.6'])
            self.assertEqual([x['name'] for x in R.ffi_libraries(root, files, 'darwin-x64')],
                             ['/usr/lib/libSystem.B.dylib', 'libSystem.dylib'])
            self.assertEqual(R.ffi_libraries(root, files, 'windows-x64'), [])
            for text in ['x = dlopen(name, { a: {} });', 'x = dlopen("libsqlite3.so.0", { a: {} });',
                         'x = dlopen(win ? "foreign.dll" : "libc.so.6", { a: {} });']:
                (root/'src/server/effects/b.js').write_text(text)
                with self.subTest(text=text), self.assertRaises(ValueError):
                    for target in ('linux-x64', 'windows-x64'):
                        R.ffi_libraries(root, files + ['src/server/effects/b.js'], target)

    def test_pe_normal_and_delay_imports(self):
        got = R.pe(pe_image(['ntdll.dll', 'KERNEL32.dll'], ['WS2_32.dll']))
        self.assertEqual(got['imports'], ['ntdll.dll', 'KERNEL32.dll'])
        self.assertEqual(got['delayImports'], ['WS2_32.dll'])
        for data in [pe_image(['ntdll.dll'], machine=0xaa64), b'MZ' + bytes(62), pe_image(['bad name.dll'])]:
            with self.assertRaises(ValueError):
                R.pe(data)

    def test_windows_measure_system_dlls_only(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            system32 = root/'System32'
            system32.mkdir()
            for name in ('ntdll.dll', 'kernel32.dll', 'ws2_32.dll'):
                (system32/name).write_bytes(pe_image())
            binary = root/'backplane-serve.exe'
            binary.write_bytes(pe_image(['ntdll.dll', 'KERNEL32.dll', 'api-ms-win-core-synch-l1-2-0.dll'], ['WS2_32.dll']))
            got = R.windows_measure(binary, 'windows-x64', [], system32)
            self.assertEqual([(e['name'], e['kind']) for e in got['dependencyGraph']],
                [('ntdll.dll', 'normal'), ('KERNEL32.dll', 'normal'), ('api-ms-win-core-synch-l1-2-0.dll', 'normal'), ('WS2_32.dll', 'delay')])
            self.assertTrue(all(n['classification'] == 'approved-system' for n in got['systemLibraries']))
            self.assertEqual(got['systemLibraries'][1]['sha256'], R.sha(system32/'kernel32.dll'))
            binary.write_bytes(pe_image(['ntdll.dll'], ['cygwin1.dll']))
            with self.assertRaisesRegex(ValueError, 'Non-system DLL'):
                R.windows_measure(binary, 'windows-x64', [], system32)
            binary.write_bytes(pe_image(['ntdll.dll', 'crypt32.dll']))
            with self.assertRaisesRegex(ValueError, 'not on this host'):
                R.windows_measure(binary, 'windows-x64', [], system32)

    def test_macho_foreign_framework_rpath_and_empty_not_approved(self):
        for name in ['/opt/homebrew/lib/libSystem.B.dylib', '@rpath/libSystem.B.dylib', '/tmp/libx.dylib', '/usr/lib/libforeign.dylib']:
            with self.assertRaises(ValueError):
                R.macho_dependencies('/service:\n\t'+name+' (compatibility version 1.0.0, current version 1.0.0)\n')
        with self.assertRaises(ValueError):
            R.macho_dependencies('/service:\n')

    def test_capture_retains_real_tool_log_and_exit(self):
        with tempfile.TemporaryDirectory(prefix='bp-runtime-parser-') as d:
            c = R.Capture(Path(d))
            self.assertEqual(c(['/usr/bin/true']), '')
            self.assertEqual(c.calls[0]['exit'], 0)
            self.assertTrue((Path(d)/c.calls[0]['log']).is_file())
            with self.assertRaises(ValueError):
                c(['/usr/bin/false'])
            self.assertEqual(c.calls[1]['exit'], 1)


if __name__ == '__main__':
    unittest.main(verbosity=2)
