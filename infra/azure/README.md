# Azure private test host

This infrastructure creates the smallest persistent host needed to serve the
static `app/dist` build for private testing. The Ubuntu VM runs nginx on
`127.0.0.1:8080`; only SSH from one explicitly supplied IPv4 `/32` is permitted
by both the Azure network security group and the guest firewall. Password SSH
and public web ingress are disabled.

## Provision

Authenticate the Azure CLI and run from the repository root:

```bash
./infra/azure/provision.sh
```

The script uses the active subscription, North Central US, a two-vCPU/one-GB
burstable VM, the current public IPv4 address, and
`~/.ssh/id_ed25519.pub` unless overridden with environment variables. The host
adds a one-GB swap file because it does not build the application. The script
prints the VM address and exact SSH/tunnel commands.

## Inspect the placeholder

Start the VM only when testing, then open the tunnel:

```bash
./infra/azure/vm-control.sh start
./infra/azure/tunnel.sh
```

Browse to
`http://127.0.0.1:18080`. The helper discovers the VM address from Azure and
keeps the tunnel open until interrupted. The loopback URL is intentional:
nginx is not exposed to the internet, and browser APIs treat localhost as a
secure context. Set `LOCAL_PORT` to override the local port.

Deallocate the VM when testing is finished:

```bash
./infra/azure/vm-control.sh stop
```

The VM has no automatic start. A defensive Azure schedule deallocates it every
day at 05:00 UTC if it was left running. A resource-group budget named
`what-is-life-test-monthly` tracks $5 of monthly cost and emails the Azure
account at 50%, 80%, and 100%. A deallocated VM stops compute charges, although
the managed disk and static public IPv4 address can continue to incur small
charges. Delete the resource group for zero persistent infrastructure when the
test environment is no longer needed; the Bicep template recreates it.

## Access changes

If the management public address changes, update both the NSG rule and UFW
before removing the old address. Azure Run Command remains available as the
recovery path if SSH becomes unavailable. Take a timestamped copy of affected
nginx, firewall, SSH, or systemd configuration before changing it.

The application is not deployed by this stack. A later deployment command will
upload a verified `app/dist` directory into a versioned
`/srv/what-is-life/releases/<build-id>` path and atomically move the `current`
symlink.

`bootstrap-host.sh` is the idempotent repair/update path for the host
configuration. It backs up nginx, SSH, UFW, fail2ban, and systemd configuration
before applying changes.
