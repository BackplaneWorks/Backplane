"""Explicit authorized-recut raw adapter; normal official verifier is untouched."""
from pathlib import Path
from authorized_inputs import reference, read, verify as bind
COMMIT = 'be904cd4cb21ad6febe17e674918c96945113d07'
TREE = '1034e9dc35fb517f1186ebcdb2d8f3351beb7a59'

def verify(receipt, receipt_sha, source, toolchain, compiler_archive, worker):
    packet = Path(receipt).resolve().parents[1]
    spec = read(packet / 'AUTHORIZED-RAW-INPUTS.json')
    identity = bind(packet / spec['ready'], spec['readySha256'], source,
        packet / spec['selection'], spec['selectionSha256'],
        packet / spec['authority'], spec['authoritySha256'],
        toolchain, compiler_archive, worker, receipt_path=receipt)
    assert identity['sourceCommit'] == COMMIT and identity['sourceTree'] == TREE
    assert identity['rawReceipt']['sha256'] == receipt_sha
    return dict(identity, rawInputBindingKey=identity['bindingKey'])
