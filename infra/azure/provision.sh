#!/usr/bin/env bash
set -euo pipefail

TASK_AZURE_LOCATION="${AZURE_LOCATION:-northcentralus}"
TASK_AZURE_RESOURCE_GROUP="${AZURE_RESOURCE_GROUP:-what-is-life-test-rg}"
TASK_AZURE_DEPLOYMENT="what-is-life-test-$(date -u +%Y%m%dT%H%M%SZ)"
TASK_AZURE_PUBLIC_IP="${MANAGEMENT_PUBLIC_IP:-$(curl --fail --silent --show-error https://api.ipify.org)}"
TASK_AZURE_SSH_PUBLIC_KEY_PATH="${SSH_PUBLIC_KEY_PATH:-${HOME}/.ssh/id_ed25519.pub}"

if [[ ! -r "${TASK_AZURE_SSH_PUBLIC_KEY_PATH}" ]]; then
  echo "SSH public key is not readable: ${TASK_AZURE_SSH_PUBLIC_KEY_PATH}" >&2
  exit 1
fi

az account show --output none
az deployment sub create \
  --name "${TASK_AZURE_DEPLOYMENT}" \
  --location "${TASK_AZURE_LOCATION}" \
  --template-file "$(dirname "${BASH_SOURCE[0]}")/main.bicep" \
  --parameters \
    location="${TASK_AZURE_LOCATION}" \
    resourceGroupName="${TASK_AZURE_RESOURCE_GROUP}" \
    adminSshPublicKey="$(<"${TASK_AZURE_SSH_PUBLIC_KEY_PATH}")" \
    managementSourceCidr="${TASK_AZURE_PUBLIC_IP}/32" \
  --query properties.outputs \
  --output json
