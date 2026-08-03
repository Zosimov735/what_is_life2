#!/usr/bin/env bash
set -euo pipefail

TASK_AZURE_RESOURCE_GROUP="${AZURE_RESOURCE_GROUP:-what-is-life-test-rg}"
TASK_AZURE_VM_NAME="${AZURE_VM_NAME:-what-is-life-test-vm}"
TASK_VM_ACTION="${1:-status}"

case "${TASK_VM_ACTION}" in
  start)
    az vm start \
      --resource-group "${TASK_AZURE_RESOURCE_GROUP}" \
      --name "${TASK_AZURE_VM_NAME}" \
      --output none
    ;;
  stop)
    az vm deallocate \
      --resource-group "${TASK_AZURE_RESOURCE_GROUP}" \
      --name "${TASK_AZURE_VM_NAME}" \
      --output none
    ;;
  status)
    ;;
  *)
    echo "Usage: $0 {start|stop|status}" >&2
    exit 2
    ;;
esac

az vm get-instance-view \
  --resource-group "${TASK_AZURE_RESOURCE_GROUP}" \
  --name "${TASK_AZURE_VM_NAME}" \
  --query "instanceView.statuses[?starts_with(code, 'PowerState/')].displayStatus | [0]" \
  --output tsv
