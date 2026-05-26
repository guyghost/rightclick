set shell := ["bash", "-cu"]

default:
    just --list

ci:
    bash scripts/dev.sh ci

check:
    bash scripts/dev.sh check

doctor:
    bash scripts/dev.sh doctor

fmt-check:
    bash scripts/dev.sh fmt-check

fmt:
    bash scripts/dev.sh fmt

clippy:
    bash scripts/dev.sh clippy

build:
    bash scripts/dev.sh build

build-release:
    bash scripts/dev.sh build-release

test:
    bash scripts/dev.sh test

test-one filter:
    bash scripts/dev.sh test-one "{{filter}}"

run:
    bash scripts/dev.sh run

install-local:
    bash scripts/dev.sh install-local
