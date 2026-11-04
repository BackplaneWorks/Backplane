"""Coverage of the download links and their native effect owners."""
from pathlib import Path
import importlib.util
import unittest
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('coverage', ROOT/'scripts/ahead-validation/coverage.py')
coverage = importlib.util.module_from_spec(spec); spec.loader.exec_module(coverage)
class Downloads(unittest.TestCase):
    def test_web_links_use_download_http_handlers(self):
        defs = coverage.definitions((ROOT/'src/web/view.bend').read_text())
        self.assertIn('D.At{"download", ""}', defs['View.fm.dl.k'])
        self.assertIn('View.fm.row.dl', defs['View.fm.row.loc'])
        self.assertIn('U.Fm.down(u, True{}, U.Fm.dir(u))', defs['View.files'])
        server = coverage.definitions((ROOT/'src/server/server.bend').read_text())
        self.assertIn('FilesHttp.serve(allowed, True{}', server['Conn.get.k'])
        self.assertIn('FilesHttp.serve(allowed, False{}', server['Conn.get.k'])
        self.assertIn('Conn.allowed(hs, p, token)', server['Conn.route.k'])
        self.assertIn('"401 Unauthorized"', server['FilesHttp.serve'])
        self.assertIn('FMP.Fm.files.safe', server['FilesHttp.serve'])
        self.assertIn('Cfg.home(cfg)', server['Hub.files.root'])
    def test_native_download_and_cancel_owners(self):
        ios = (ROOT/'mobile/ios/Backplane/FilesView.swift').read_text()
        android = (ROOT/'mobile/android/app/src/main/java/dev/backplane/mobile/FilesView.kt').read_text()
        self.assertIn('download.start(model, path, name)', ios)
        self.assertIn('download.cancel()', ios)
        self.assertIn('ShareSheet(url: f.url)', ios)
        self.assertIn('ActivityResultContracts.CreateDocument', android)
        self.assertIn('current.fetch(p.url, ctx, uri)', android)
        self.assertIn('onCancel = { transfer?.cancel() }', android)
        self.assertIn('instanceFollowRedirects = false', android)
    def test_existing_literal_handlers_still_resolve(self):
        report = coverage.inventory(coverage.Source(root=ROOT))
        # No invented action for download anchors: the browser owns href.
        new = [r for r in report.get('rows', []) if r.get('action', '').startswith('fm-download')]
        self.assertEqual(new, [])
if __name__ == '__main__': unittest.main()
