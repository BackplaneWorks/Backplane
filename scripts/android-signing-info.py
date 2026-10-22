#!/usr/bin/env python3
"""Read the configured keystore's public certificate fingerprint; never build."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def need(ok, message):
    if not ok: raise ValueError(message)


def public_signer(env, source):
    need(re.fullmatch('[0-9a-f]{40}', source), 'Frozen product commit required')
    workflow = env.get('GITHUB_SHA', '')
    need(re.fullmatch('[0-9a-f]{40}', workflow), 'Actual workflow metadata commit required')
    encoded = env.get('BACKPLANE_ANDROID_KEYSTORE_BASE64', '')
    password = env.get('KEYSTORE_PASSWORD', '')
    alias = env.get('KEY_ALIAS', '')
    need(encoded and password and alias, 'Configured signing inputs required')
    raw = base64.b64decode(''.join(encoded.split()), validate=True)
    need(0 < len(raw) <= 8 * 1024**2, 'Keystore size refused')
    keytool = shutil.which('keytool')
    openssl = shutil.which('openssl')
    need(keytool and openssl, 'Public certificate tools unavailable')
    # The export process receives only its required password, not the encoded
    # store, unrelated release secrets or a private-key password.
    child = {key: value for key, value in env.items() if key in
             ('PATH', 'JAVA_HOME', 'LANG', 'LC_ALL', 'TMPDIR', 'SYSTEMROOT')}
    child['KEYSTORE_PASSWORD'] = password
    with tempfile.TemporaryDirectory(prefix='bp-public-signer-') as tmp:
        directory = Path(tmp); directory.chmod(0o700)
        store = directory / 'configured-keystore'
        fd = os.open(store, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        with os.fdopen(fd, 'wb') as f: f.write(raw)
        exported = subprocess.run([keytool, '-exportcert', '-alias', alias, '-keystore', str(store),
                                   '-storepass:env', 'KEYSTORE_PASSWORD'],
                                  env=child, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, timeout=45, check=False)
        need(exported.returncode == 0 and 32 <= len(exported.stdout) <= 1024**2,
             'Configured certificate export failed')
        certificate = exported.stdout
        validated = subprocess.run([openssl, 'x509', '-inform', 'DER', '-noout'], input=certificate,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15,
                                   env={k:v for k,v in child.items() if k != 'KEYSTORE_PASSWORD'}, check=False)
        need(validated.returncode == 0, 'Exported public certificate is not valid DER')
        fingerprint = hashlib.sha256(certificate).hexdigest()
    need(not directory.exists(), 'Private temporary store cleanup failed')
    return {'schema': 1, 'kind': 'backplane-configured-android-public-signer',
            'status': 'PASS_PUBLIC_CERTIFICATE_IDENTITY_ONLY', 'sourceCommit': source,
            'workflowSourceCommit': workflow, 'version': '0.12.1', 'channel': 'release',
            'certificateSha256': fingerprint, 'keystoreRemovedBeforeReceipt': True,
            'privateInputsRetained': False, 'appBuildExecuted': False,
            'artifactRuntimeAccepted': False, 'published': False}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    need(a.out.is_absolute() and not a.out.exists(), 'Fresh absolute public receipt path required')
    receipt = public_signer(os.environ, a.source)
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt, sort_keys=True))


if __name__ == '__main__':
    try: main()
    except (ValueError, KeyError, OSError, subprocess.SubprocessError):
        raise SystemExit('android-signing-info: public identity preflight failed; no private material retained or logged')
