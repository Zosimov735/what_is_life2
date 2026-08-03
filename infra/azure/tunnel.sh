#!/usr/bin/env bash
set -euo pipefail

TASK_AZURE_RESOURCE_GROUP="${AZURE_RESOURCE_GROUP:-what-is-life-test-rg}"
TASK_AZURE_VM_NAME="${AZURE_VM_NAME:-what-is-life-test-vm}"
TASK_AZURE_ADMIN_USERNAME="${AZURE_ADMIN_USERNAME:-azuredev}"
TASK_AZURE_LOCAL_PORT="${LOCAL_PORT:-18080}"
TASK_AZURE_PUBLIC_IP="$(az vm list-ip-addresses \
  --resource-group "${TASK_AZURE_RESOURCE_GROUP}" \
  --name "${TASK_AZURE_VM_NAME}" \
  --query '[0].virtualMachine.network.publicIpAddresses[0].ipAddress' \
  --output tsv)"

if [[ -z "${TASK_AZURE_PUBLIC_IP}" ]]; then
  echo "Azure did not return a public IP for ${TASK_AZURE_VM_NAME}." >&2
  exit 1
fi

echo "Open http://127.0.0.1:${TASK_AZURE_LOCAL_PORT} while this tunnel is running."
exec ssh \
  -o ExitOnForwardFailure=yes \
  -N \
  -L "${TASK_AZURE_LOCAL_PORT}:127.0.0.1:8080" \
  "${TASK_AZURE_ADMIN_USERNAME}@${TASK_AZURE_PUBLIC_IP}"
