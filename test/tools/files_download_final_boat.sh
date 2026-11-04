#!/bin/bash
set -eu
bash test/tools/files_download_boat.sh
printf 'FOCUSED_DOWNLOAD_GATE_PASSED\n'
# Required monolithic proof attempt stays under the same hard cap; its
# failure must never replace the passing, scoped gate with a full-pass claim.
bash test/tools/files_download_fullproof.sh
