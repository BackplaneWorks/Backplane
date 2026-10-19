#!/usr/bin/env python3
"""Negative cases for the native catalog compiler; executable smoke tests live in test/native."""
import importlib.util
from pathlib import Path
import sys
import unittest
import contextlib
import io
import tempfile
import plistlib
from unittest.mock import patch

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('bp_i18n_gen', root / 'scripts/i18n-gen.py')
gen = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = gen
spec.loader.exec_module(gen)


def catalog(text):
    return gen.parse_catalog(text, "native compiler test")


def info_template():
    return plistlib.dumps({'NSCameraUsageDescription': '@NSCameraUsageDescription@',
                          'CFBundleURLTypes': [{'CFBundleURLSchemes': ['backplane']}]}).decode('utf-8')


class NativeCodegen(unittest.TestCase):
    def test_ios_resource_roundtrip_escaping_and_fallback(self):
        text = 'Read "code" & <camera>\n次の行'
        cats = {'en': {'native.permission.camera': [gen.Text(text)]}, 'ja': {}}
        outputs = gen.generate_ios_resources(cats, info_template())
        for name, value in outputs.items():
            parsed = plistlib.loads(value.encode('utf-8'))
            self.assertEqual(parsed['NSCameraUsageDescription'], text, name)
        self.assertEqual(plistlib.loads(outputs['mobile/ios/Info.plist'].encode())['CFBundleURLTypes'],
                         [{'CFBundleURLSchemes': ['backplane']}])

    def test_ios_resources_reject_runtime_formatting(self):
        for message in ['Use {name}', '{n, plural, other {# cameras}}', '{who, select, other {Camera}}']:
            cats = {'en': catalog('native.permission.camera = ' + message), 'ja': {}}
            with self.assertRaises(gen.CatalogError):
                gen.generate_ios_resources(cats, info_template())

    def test_ios_resources_reject_template_key_gaps(self):
        cats = {'en': catalog('native.permission.camera = Camera'), 'ja': {}}
        for text in [info_template().replace('@NSCameraUsageDescription@', 'Camera'),
                     info_template().replace('@NSCameraUsageDescription@', '@UnknownKey@'),
                     info_template().replace('@NSCameraUsageDescription@', '@NSCameraUsageDescription@@NSCameraUsageDescription@'),
                     info_template().replace('<string>@NSCameraUsageDescription@</string>', '<array><string>@NSCameraUsageDescription@</string></array>')]:
            with self.assertRaises(gen.CatalogError):
                gen.generate_ios_resources(cats, text)

    def test_ios_resources_require_nonempty_source(self):
        for entries in [{}, {'native.permission.camera': []}]:
            with self.assertRaises(gen.CatalogError):
                gen.generate_ios_resources({'en': entries, 'ja': {}}, info_template())

    def test_catalog_metadata_helpers_cannot_shadow_a_message(self):
        cats = {'en': catalog('catalog.rev = Source'), 'ja': catalog('catalog.rev = 翻訳')}
        with self.assertRaises(gen.CatalogError):
            gen.generate(cats)

    def test_duplicate_native_key(self):
        with self.assertRaises(gen.CatalogError):
            gen.native_keys('common.save\ncommon.save')

    def test_missing_export(self):
        cats = {'en': catalog('common.save = Save'), 'ja': {}}
        with self.assertRaises(gen.CatalogError):
            gen.generate_native(cats, ['common.missing'], True)

    def test_kotlin_literal_does_not_interpolate_user_sigils(self):
        self.assertEqual(gen.native_literal('$skill \\" \\n', False), '"\\$skill \\\\\\" \\\\n"')

    def test_native_fallback_keeps_english_plural(self):
        cats = {'en': catalog('files.count = {n, plural, one {# file} other {# files}}'), 'ja': {}}
        for swift in (True, False):
            text = gen.generate_native(cats, ['files.count'], swift)
            self.assertEqual(text.count('arg0 == 1' + ('' if swift else 'uL')), 2)

    def test_native_exact_integer_range_is_explicit(self):
        cats = {'en': catalog('files.count = {n, plural, =18446744073709551616 {huge} other {#}}'), 'ja': {}}
        with self.assertRaises(gen.CatalogError):
            gen.generate_native(cats, ['files.count'], True)

    def test_native_rejects_placeholder_changes(self):
        cats = {'en': catalog('common.save = Save {name}'), 'ja': catalog('common.save = {other}を保存')}
        with self.assertRaises(gen.CatalogError):
            gen.generate_native(cats, ['common.save'], False)

    def test_check_rejects_stale_native_outputs(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(gen, 'ROOT', Path(temp)), patch.object(sys, 'argv', ['i18n-gen.py']):
            folder = Path(temp)
            (folder / 'i18n').mkdir()
            (folder / 'i18n/en.msg').write_text('common.save = Save\nnative.permission.camera = Camera\n')
            (folder / 'i18n/ja.msg').write_text('common.save = 保存\nnative.permission.camera = カメラ\n')
            (folder / 'i18n/native.keys').write_text('common.save\n')
            (folder / 'mobile/ios').mkdir(parents=True)
            (folder / 'mobile/ios/Info.plist.in').write_text(info_template())
            for target in ['mobile/ios/Shared/NativeMessages.swift', 'mobile/android/app/src/main/java/dev/backplane/mobile/NativeMessages.kt',
                           'mobile/ios/Info.plist', 'mobile/ios/Backplane/en.lproj/InfoPlist.strings',
                           'mobile/ios/Backplane/ja.lproj/InfoPlist.strings']:
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(gen.main(), 0)
                output = folder / target
                output.write_text(output.read_text() + '// deliberate stale output\n')
                with patch.object(sys, 'argv', ['i18n-gen.py', '--check']), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(gen.main(), 1)



if __name__ == '__main__':
    unittest.main()
