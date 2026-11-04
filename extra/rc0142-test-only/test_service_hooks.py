"""Compile-only hooks for the separate experimental TEST producer."""
import json
import fcntl
import importlib.util
import os
from pathlib import Path
import shutil

import raw_inputs


class TestHooks:
    def __init__(self, producer, request, base):
        self.p = producer
        inputs = request['experimentalInputs']
        paths = {name: producer.bound(inputs[name], base) for name in
                 ('receipt', 'toolchain', 'compilerArchive', 'worker')}
        self.identity = raw_inputs.verify(paths['receipt'], inputs['receipt']['sha256'],
            producer.ROOT, paths['toolchain'], paths['compilerArchive'], paths['worker'])
        self.root = paths['receipt'].parent
        self.compiler_manifest = paths['toolchain']
        producer.require(request['toolchain']['bend'] == {
            **producer.reference(self.compiler_manifest), 'version': 'composite-settle-41262875',
            'kind': 'source-compiler-manifest-not-an-official-executable'},
            'TEST compiler descriptor must name its actual manifest, never official bend')

    def compiler(self):
        return self.compiler_manifest

    def preemission(self):
        raw = self.p.read(self.root / 'LOWMEM-EMISSION.json')
        # This key binds validated experimental source/tool/raw facts. It is
        # not scripts/inputs-key.sh's official compiler key and is never
        # offered to the normal release verified_preemission consumer.
        return self.root, {**raw, 'serverEmitKey': self.identity['rawInputBindingKey']}

    def link(self, args, log, env):
        self.p.require(args[0] == 'scripts/build-js.sh' and args[1] == 'src/server/main.bend',
                       'Only the retained hub compile-only continuation is supported')
        script = Path(__file__).with_name('compile-test-retained.sh')
        self.p.require(script.is_file(), 'Prepared test linker missing')
        tools = Path(__file__).resolve().parents[1]
        manifest = self.p.read(tools / 'TEST-TOOLING.json')
        self.p.require(self.p.digest(script) == manifest['members']['scripts/compile-test-retained.sh'],
                       'Exact TEST transform/link suffix changed')
        # The original guard CLI intentionally admits only build-js.sh. Do
        # not weaken that contract or pretend this external wrapper is it.
        # This separate TEST consumer pins its own fixed argv/hash, then uses
        # the unchanged memory admission and native lock primitives.
        spec = importlib.util.spec_from_file_location('frozen_test_memory_admission',
                    self.p.ROOT / 'scripts/heavy-guard.py')
        guard = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(guard)
        self.p.require(guard.FLOORS['compile-js'] == 3 * 1024**3
                       and guard.verify_shard_cgroup(), 'Exact <=3.7GB/swap0 TEST scope required')
        lock = os.open(guard.lock_path(), os.O_CREAT | os.O_RDWR | getattr(os, 'O_NOFOLLOW', 0), 0o666)
        # Normal retained-js validates/copies the raw bytes again; this script
        # contains only the frozen build-js transform/link suffix, no emitter.
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.p.require(guard.admit('compile-js'), 'Frozen 3GiB admission/8GiB host reserve failed')
            self.p.write(Path(log).with_name('test-admission.json'), {
                'testOnly': True, 'profile': 'external-test-retained-link',
                'originalGuardSha256': self.p.digest(self.p.ROOT / 'scripts/heavy-guard.py'),
                'floorBytes': guard.FLOORS['compile-js'], 'hostReserveBytes': guard.HOST_RESERVE,
                'cgroupLimitMaxBytes': 3700000000, 'swapMaxBytes': 0,
                'nativeLockHeld': True, 'scriptSha256': self.p.digest(script),
                'releaseGuardCLIChanged': False})
            return self.p.run(['bash', script, *args[1:]], log,
                dict(env, BACKPLANE_TEST_PRODUCT=str(self.p.ROOT)))
        finally:
            os.close(lock)

    def provenance(self):
        return {'testOnly': True, 'releaseQualified': False,
                'stockEquivalent': False, 'experimentalInputs': self.identity,
                'buildProvenance': {'compilerMode': 'composite-settle-41262875',
                    'producerWorkId': 'w-t1792701529-415584-1614985',
                    'releaseBuildApproved': False},
                'qualificationScope': 'Exact experimental raw inputs and configured target link only; '
                                      'no stock output comparison or release acceptance',
                'publication': 'HOLD_FOR_OUTPUT_REVIEW_AND_USER_CONFIRMATION'}
