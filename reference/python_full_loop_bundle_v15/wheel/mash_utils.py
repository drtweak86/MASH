"""
Optional 'wheel' helper (pure python).
Package later if you want offline distribution.
"""
from __future__ import annotations
from pathlib import Path
import subprocess

def is_installed_rpm(name: str) -> bool:
    return subprocess.run(["rpm","-q",name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0

def append_once(path: Path, line: str):
    txt = path.read_text() if path.exists() else ""
    if line not in txt:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(txt + ("\n" if txt and not txt.endswith("\n") else "") + line + "\n")
