set shell := ["bash", "-cu"]

default:
    just help

help:
    bash scripts/dev.sh help

ci:
    bash scripts/dev.sh ci

pre-commit:
    bash scripts/dev.sh pre-commit

pre-push:
    bash scripts/dev.sh pre-push

check:
    bash scripts/dev.sh check

quick:
    bash scripts/dev.sh quick

script-check:
    bash scripts/dev.sh script-check

doctor:
    bash scripts/dev.sh doctor

rust-version:
    bash scripts/dev.sh rust-version

fmt-check:
    bash scripts/dev.sh fmt-check

fmt:
    bash scripts/dev.sh fmt

clippy:
    bash scripts/dev.sh clippy

lint:
    bash scripts/dev.sh lint

build:
    bash scripts/dev.sh build

build-release:
    bash scripts/dev.sh build-release

test:
    bash scripts/dev.sh test

doc-test:
    bash scripts/dev.sh doc-test

test-list *filters:
    bash scripts/dev.sh test-list {{filters}}

test-one +args:
    bash scripts/dev.sh test-one {{args}}

test-many +filters:
    bash scripts/dev.sh test-many {{filters}}

run:
    bash scripts/dev.sh run

install-local:
    bash scripts/dev.sh install-local
