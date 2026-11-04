"""Tiny remote controls of the actual intake's metadata validation."""
import copy
import unittest
from seed_intake import SOURCE, validate


class Controls(unittest.TestCase):
    def setUp(self):
        self.release = {'id': 1, 'draft': True, 'published_at': None,
                        'target_commitish': SOURCE, 'assets': [{'id': 2}]}
        self.asset = {'id': 2, 'name': 'seed.tar.gz', 'state': 'uploaded',
                      'size': 3, 'digest': 'sha256:'+'a'*64}

    def check(self, release=None, asset=None):
        validate(release or self.release, asset or self.asset, 1, 2, 'a'*64, 3, 'seed.tar.gz')

    def test_valid(self):
        self.check()

    def test_published_or_foreign_draft_rejected(self):
        for key, wrong in [('id', 3), ('draft', False), ('published_at', 'now'),
                           ('target_commitish', '0'*40), ('assets', [{'id': 3}])]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.check(release=dict(self.release, **{key: wrong}))

    def test_foreign_incomplete_or_hash_mismatched_asset_rejected(self):
        for key, wrong in [('id', 3), ('name', 'different.tar.gz'), ('state', 'new'),
                           ('size', 4), ('digest', 'sha256:'+'b'*64)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.check(asset=dict(self.asset, **{key: wrong}))


if __name__ == '__main__':
    unittest.main(verbosity=2)
