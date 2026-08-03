targetScope = 'subscription'

@description('Azure region for the private test host.')
param location string = 'northcentralus'

@description('Resource group containing the test host.')
param resourceGroupName string = 'what-is-life-test-rg'

@description('Short resource-name prefix.')
param namePrefix string = 'what-is-life-test'

@description('Linux administrator and deployment account.')
param adminUsername string = 'azuredev'

@description('SSH public key for the administrator account.')
param adminSshPublicKey string

@description('Only this IPv4 CIDR may reach SSH, for example 203.0.113.8/32.')
param managementSourceCidr string

@description('Small burstable size is sufficient because the application is static.')
param vmSize string = 'Standard_B2ats_v2'

var deploymentTags = {
  project: 'what-is-life'
  environment: 'test'
  managedBy: 'bicep'
}

resource resourceGroup 'Microsoft.Resources/resourceGroups@2024-11-01' = {
  name: resourceGroupName
  location: location
  tags: deploymentTags
}

module testHost 'test-host.bicep' = {
  name: 'what-is-life-test-host'
  scope: resourceGroup
  params: {
    location: location
    namePrefix: namePrefix
    adminUsername: adminUsername
    adminSshPublicKey: adminSshPublicKey
    managementSourceCidr: managementSourceCidr
    vmSize: vmSize
    tags: deploymentTags
  }
}

output resourceGroupName string = resourceGroup.name
output vmName string = testHost.outputs.vmName
output publicIpAddress string = testHost.outputs.publicIpAddress
output sshCommand string = testHost.outputs.sshCommand
output tunnelCommand string = testHost.outputs.tunnelCommand
