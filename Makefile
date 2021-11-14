.PHONY: all build check test test-all fmt clippy clean bench run-example doc

CARGO ?= cargo
PYTHON ?= python3

all: check test

build:
	$(CARGO) build --workspace

build-release:
	$(CARGO) build --workspace --release

check:
	$(CARGO) check --workspace --all-targets

test:
	$(CARGO) test --workspace -- --nocapture

test-all: test test-python

test-python:
	pytest python/tests -v || $(PYTHON) -m unittest discover -s python/tests -p "test_*.py"

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

clean:
	$(CARGO) clean
	rm -rf target build dist *.egg-info

bench:
	$(CARGO) bench --workspace

doc:
	$(CARGO) doc --workspace --no-deps --open

run-example:
	$(CARGO) run --bin torque-cli -- start --config examples/gateway.yaml
