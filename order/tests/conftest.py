import os
import pathlib
import sys
import tempfile

HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(HERE.parent))
_data = tempfile.mkdtemp(prefix="fab-test-")
os.environ.update({"FAB_DATA": _data, "FAB_VENDOR": str(HERE / "fakevendor"), "FAB_ENV_FILE": "/nonexistent",
                   "FAB_ADMIN_TOKEN": "admin-token-for-tests-0123456789", "FAB_PUBLIC_URL": "http://testserver"})
os.environ.pop("STRIPE_KEY", None)
