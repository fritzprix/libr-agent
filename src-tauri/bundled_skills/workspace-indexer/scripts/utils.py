import hashlib
import re
from pathlib import Path

def get_source_from_md(md_path: Path) -> str | None:
    """Read first 5 lines of markdown to extract the source path."""
    if not md_path.exists():
        return None
    try:
        with open(md_path, "r", encoding="utf-8", errors="ignore") as f:
            for _ in range(5):
                line = f.readline()
                if not line:
                    break
                # matches: > Source: `path`
                m = re.search(r">\s*Source:\s*`([^`]+)`", line)
                if m:
                    return m.group(1).strip()
    except Exception:
        pass
    return None

def get_path_hash(src: Path) -> str:
    """Generate a short hash based on the absolute source path."""
    return hashlib.sha256(str(src.resolve()).encode("utf-8")).hexdigest()[:4]
