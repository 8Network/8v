# Dockerfile with known hadolint violations for testing.
#
# Violations included:
#   DL3007 — Using latest is prone to errors
#   DL3008 — Pin versions in apt-get install
#   DL3015 — Avoid additional packages with apt-get install (missing --no-install-recommends)
#   DL3009 — Delete the apt-get lists after installing

FROM ubuntu:latest

RUN apt-get update && apt-get install -y curl
