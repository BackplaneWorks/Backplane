"""Settings, all from the environment (systemd's EnvironmentFile on the server).

Secrets never live in the repository: API keys come from FAB_ENV_FILE or the
process environment, and the fab's pricing pack is installed separately
(FAB_VENDOR, a directory holding a `vendor` package; see vendor_api.py).
"""
import os
import pathlib


def _load_env_file(path):
    """KEY=VALUE lines into os.environ (without overriding what is already set)."""
    try:
        text = pathlib.Path(path).read_text()
    except OSError:
        return
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        os.environ.setdefault(k.strip(), v.strip().strip('"').strip("'"))


_load_env_file(os.environ.get("FAB_ENV_FILE", "/etc/fab/env"))


def env(name, default=""):
    return os.environ.get(name, default)


DATA = pathlib.Path(env("FAB_DATA", "/srv/fab/data"))
VENDOR = env("FAB_VENDOR", "")
# where people reach the server (Buy now links, Stripe's return URLs)
PUBLIC_URL = env("FAB_PUBLIC_URL", "http://127.0.0.1:8090").rstrip("/")

STRIPE_KEY = env("STRIPE_KEY")
# The pilot: checkout runs on a Stripe test key, so no card is charged and
# nothing is ordered; every page says so and points to ordering the exported
# files yourself (FAB_PILOT=1 forces it, 0 turns it off)
PILOT = env("FAB_PILOT", "1" if STRIPE_KEY.startswith("sk_test_") else "0") == "1"
PILOT_NOTE = ("Pilot: ordering through Backplane is still being tested. This checkout is a test and your card will not be "
              "charged. To try it, use the card 4242 4242 4242 4242 with any future date and CVC. To get your parts made now, "
              "order them from any fabricator with the files Backplane exported to the mfg folder of your project.")
MOUSER_KEY = env("MOUSER_API_KEY")
TI_KEY = env("TI_API_KEY")
TI_SECRET = env("TI_API_SECRET")
# the order list at /admin
ADMIN_TOKEN = env("FAB_ADMIN_TOKEN")

FEE = float(env("FAB_FEE", "0.10"))
FEE_CNC = float(env("FAB_FEE_CNC", "0.20"))
QUOTE_DAYS = int(env("FAB_QUOTE_DAYS", "3"))
CURRENCY = "usd"

MAX_UPLOAD = int(env("FAB_MAX_UPLOAD", str(50 * 1024 * 1024)))
MAX_UNPACKED = int(env("FAB_MAX_UNPACKED", str(200 * 1024 * 1024)))
MAX_FILES = int(env("FAB_MAX_FILES", "400"))
# unpaid quotes and their files go after this many days
KEEP_DAYS = int(env("FAB_KEEP_DAYS", "7"))
# analyses running at once (each in its own sandboxed process)
WORKERS = int(env("FAB_WORKERS", "2"))
# behind nginx: trust X-Real-IP from these peers only
TRUSTED_PROXIES = set(env("FAB_TRUSTED_PROXIES", "127.0.0.1,::1").split(","))
VERSION = "1"
