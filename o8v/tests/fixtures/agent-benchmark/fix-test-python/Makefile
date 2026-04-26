.PHONY: test install

install:
	pip install -e ".[dev]" -q

test: install
	python -m pytest tests/ -v
