"""TEST-only, exact-source Windows CRT classification around unchanged ABI checks."""
import argparse
import ctypes
import importlib.util
import json
from pathlib import Path
import shutil
import sys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('factory', 'source-root', 'build', 'producer', 'out', 'windows-runtime'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--producer-sha256', required=True)
    a = p.parse_args()
    assert sys.platform == 'win32', 'Windows native catalog observation only'
    spec = importlib.util.spec_from_file_location('test_runtime_factory', a.factory)
    runtime = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runtime)
    source = a.source_root.resolve()
    assert runtime.sha(source / 'src/server/effects/zip.js') == '1cfd240f87e11a4a80fa6d910d166f683bdce5165570e2179356131129a91ff8'
    assert runtime.FFI_APPROVED['windows'] == {'kernel32.dll'}
    assert runtime.windows_system('msvcrt.dll') is True
    # One reviewed source declaration, not a wildcard/native-loader override.
    original_ffi = runtime.ffi_libraries
    runtime.FFI_APPROVED['windows'] = {'kernel32.dll', 'msvcrt.dll'}
    def ffi(root, files, target):
        result = original_ffi(root, files, target)
        crt = [x for x in result if x['name'] == 'msvcrt.dll']
        assert target == 'windows-x64' and crt == [{'name': 'msvcrt.dll', 'declaredBy': ['src/server/effects/zip.js']}]
        return result
    runtime.ffi_libraries = ffi
    original_measure = runtime.windows_measure
    def measure(binary, target, dynamic, system32=None):
        assert system32 is None and target == 'windows-x64'
        get_system = ctypes.windll.kernel32.GetSystemDirectoryW
        get_system.argtypes = [ctypes.c_wchar_p, ctypes.c_uint]
        get_system.restype = ctypes.c_uint
        buffer = ctypes.create_unicode_buffer(32768)
        count = get_system(buffer, len(buffer))
        assert 0 < count < len(buffer)
        directory = Path(buffer.value).resolve(strict=True)
        dll = directory / 'msvcrt.dll'
        assert dll.is_file() and not dll.is_symlink()
        image = runtime.pe(dll.read_bytes())
        assert image['machine'] == '0x8664'
        # Resolve the declared export from the absolute OS DLL; do not call it.
        observed = ctypes.WinDLL(str(dll))
        assert getattr(observed, '_get_osfhandle', None) is not None
        result = original_measure(binary, target, dynamic, directory)
        if not any(x.get('name') == 'msvcrt.dll' for x in result['systemLibraries']):
            result['systemLibraries'].append({'name': 'msvcrt.dll', 'path': str(dll),
                'sha256': runtime.sha(dll), 'bytes': dll.stat().st_size,
                'architecture': target, 'classification': 'approved-system', 'redistributed': False})
        result['dependencyGraph'].append({'from': 'service', 'name': 'msvcrt.dll',
            'kind': 'dynamic', 'resolvedSystemPath': str(dll), 'system': True,
            'classification': 'approved-system'})
        evidence = a.out / 'runtime/runtime-evidence'
        shutil.copyfile(__file__, evidence / 'TEST-NATIVE-CATALOG.py')
        disposition = {'schema': 1, 'testOnly': True, 'releaseQualified': False,
            'library': 'msvcrt.dll', 'declaredBy': 'src/server/effects/zip.js',
            'sourceSha256': runtime.sha(source / 'src/server/effects/zip.js'),
            'classification': 'approved-system', 'redistributed': False,
            'path': str(dll), 'sha256': runtime.sha(dll), 'bytes': dll.stat().st_size,
            'architecture': target, 'observedExport': '_get_osfhandle',
            'originalFactorySHA256': runtime.sha(a.factory),
            'catalogAdapterSHA256': runtime.sha(Path(__file__)),
            'scope': 'Exact frozen zip effect declaration and actual System32 PE/export; all existing ABI/source/hash checks retained'}
        (evidence / 'TEST-MSVCRT-DISPOSITION.json').write_text(json.dumps(disposition, indent=2) + '\n')
        result['testCatalogDisposition'] = disposition
        return result
    runtime.windows_measure = measure
    result = runtime.prepare(a)
    print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
