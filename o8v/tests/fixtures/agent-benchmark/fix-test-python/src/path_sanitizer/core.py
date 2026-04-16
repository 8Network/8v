"""Safe path joining.

`safe_join(base, *parts)` joins a trusted base directory with untrusted
user-supplied path components and refuses any result that escapes `base`.

Used by file-serving endpoints that accept user input as a path suffix.
"""

import os


def safe_join(base: str, *parts: str) -> str:
    """Join `base` with `parts`; raise ValueError if the result escapes `base`.

    Args:
        base: Trusted base directory (absolute or relative).
        parts: Untrusted path components to append.

    Returns:
        An absolute, normalized path that lives under `base`.

    Raises:
        ValueError: If the resulting path escapes `base`.
    """
    base = os.path.abspath(base)
    joined = os.path.join(base, *parts)
    joined = os.path.normpath(joined)
    if ".." in joined:
        raise ValueError(f"path traversal detected: {joined!r}")
    return joined
