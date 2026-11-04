"""Compile-only hooks for the separate experimental TEST producer."""
import json
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
        # Normal retained-js validates/copies the raw bytes again; this script
        # contains only the frozen build-js transform/link suffix, no emitter.
        return self.p.run(['bash', script, *args[1:]], log,
            dict(env, BACKPLANE_TEST_PRODUCT=str(self.p.ROOT)))

    def provenance(self):
        return {'testOnly': True, 'releaseQualified': False,
                'stockEquivalent': False, 'experimentalInputs': self.identity,
                'buildProvenance': {'compilerMode': 'composite-settle-41262875',
                    'producerWorkId': 'w-t1792701529-415584-1614985',
                    'releaseBuildApproved': False},
                'qualificationScope': 'Exact experimental raw inputs and configured target link only; '
                                      'no stock output comparison or release acceptance',
                'publication': 'HOLD_FOR_OUTPUT_REVIEW_AND_USER_CONFIRMATION'}
