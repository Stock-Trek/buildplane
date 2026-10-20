#!/usr/bin/env bash
#
# Bootstrap the Terraform remote state bucket for a fresh AWS account.
#
# `bootstrap` stores its state in the very S3 bucket it creates
# (state.stock-trek.com), so a new account cannot run `terraform init` until
# that bucket exists. This script breaks the cycle by applying with a local
# backend and then migrating the local state into the bucket it just created.
#
# It only needs to be run once, before the first `terraform -chdir=bootstrap
# apply`. Subsequent runs use the S3 backend as normal.
set -euo pipefail

BOOTSTRAP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

command -v terraform >/dev/null 2>&1 || {
  echo "error: terraform is not installed or not on PATH" >&2
  exit 1
}

echo "==> Initialising with a local backend (the S3 backend bucket does not exist yet)"
terraform -chdir="${BOOTSTRAP_DIR}" init -backend=false -input=false

echo "==> Creating the remote state bucket"
terraform -chdir="${BOOTSTRAP_DIR}" apply -input=false -auto-approve -target=module.bucket_bootstrap

echo "==> Migrating the local state into the newly created S3 backend"
terraform -chdir="${BOOTSTRAP_DIR}" init -input=false -migrate-state -force-copy

echo "==> Applying the remaining bootstrap resources"
terraform -chdir="${BOOTSTRAP_DIR}" apply -input=false -auto-approve

echo "==> Bootstrap complete"
