"""Lossless current-build image selection."""
from pathlib import Path
import runpy
runpy.run_path(str(Path(__file__).with_name('current-finish.py')),run_name='__main__')
