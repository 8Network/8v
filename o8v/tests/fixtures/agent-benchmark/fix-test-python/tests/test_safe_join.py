import pytest

from path_sanitizer import safe_join


def test_joins_simple_relative_path():
    result = safe_join("/safe/base", "file.txt")
    assert result == "/safe/base/file.txt"


def test_joins_nested_subdir():
    result = safe_join("/safe/base", "sub", "file.txt")
    assert result == "/safe/base/sub/file.txt"


def test_rejects_parent_traversal():
    with pytest.raises(ValueError):
        safe_join("/safe/base", "..", "etc", "passwd")


def test_rejects_absolute_path_component():
    # User-supplied absolute path must not escape the base.
    with pytest.raises(ValueError):
        safe_join("/safe/base", "/etc/passwd")


def test_accepts_filename_with_dots():
    # A filename that merely contains ".." as a substring is legal.
    result = safe_join("/safe/base", "foo..bar.txt")
    assert result == "/safe/base/foo..bar.txt"
