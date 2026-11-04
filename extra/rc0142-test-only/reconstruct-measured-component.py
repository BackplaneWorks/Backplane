"""Restore a retained measured TEST component; no new DLL measurement or link."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import sys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('factory', 'retained', 'source-root', 'build', 'producer', 'out', 'windows-runtime'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--producer-sha256', required=True)
    a = p.parse_args()
    assert sys.platform == 'win32'
    spec = importlib.util.spec_from_file_location('retained_component_factory', a.factory)
    runtime = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runtime)
    prior = a.retained.resolve() / 'service-component'
    expected = json.loads((prior / 'component/SERVICE.json').read_text())
    evidence = prior / 'runtime/runtime-evidence'
    measurement = json.loads((evidence / 'system-dependencies.json').read_text())
    assert expected['testOnly'] is True and expected['releaseQualified'] is False
    assert measurement['sourceCommit'] == 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4'
    assert measurement['sourceTree'] == 'ec6c85b4a8b62a0227f09151a9ec52a362b20ae2'
    assert measurement['target'] == 'windows-x64'
    assert measurement['binarySha256'] == runtime.sha(a.build / 'backplane-serve.exe')
    assert runtime.sha(a.factory) == runtime.sha(evidence / 'SERVICE-RUNTIME-PRODUCER.py')
    runtime.FFI_APPROVED['windows'] = {'kernel32.dll', 'msvcrt.dll'}

    def retained_measurement(binary, target, dynamic):
        assert target == measurement['target'] and dynamic == measurement['dynamicLoads']
        assert runtime.sha(binary) == measurement['binarySha256']
        destination = a.out / 'runtime/runtime-evidence'
        for name in ('TEST-NATIVE-CATALOG.py', 'TEST-MSVCRT-DISPOSITION.json'):
            shutil.copyfile(evidence / name, destination / name)
        return copy.deepcopy(measurement)

    runtime.windows_measure = retained_measurement
    runtime.prepare(a)
    actual = json.loads((a.out / 'component/SERVICE.json').read_text())
    # The recovery may not create a new measured payload or substitute bytes.
    assert actual == expected, 'Retained component reconstruction differs'
    assert runtime.sha(a.out / 'RUNTIME-QUALIFICATION.json') == runtime.sha(prior / 'RUNTIME-QUALIFICATION.json')
    print(json.dumps({'status': 'EXACT_RETAINED_COMPONENT_RECONSTRUCTED',
        'componentSHA256': runtime.sha(a.out / 'component/SERVICE.json'),
        'abiMeasuredAgain': False, 'binaryLinkedAgain': False, 'releaseQualified': False}))


if __name__ == '__main__':
    main()
