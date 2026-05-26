set shell := ["bash", "-cu"]

default:
    just --list

ci:
    bash scripts/dev.sh ci

check:
    bash scripts/dev.sh check

fmt-check:
    bash scripts/dev.sh fmt-check

clippy:
    bash scripts/dev.sh clippy

test:
    bash scripts/dev.sh test

run:
    bash scripts/dev.sh run

install-local:
    bash scripts/dev.sh install-local
