// Root deployment for RIB edge + core infra (initial skeleton)
// Parameter defaults chosen for dev/prototype; override via parameter file or CLI.

// (Removed unused location param; all current resources are global.)

@description('Environment name (e.g. dev, prod) used in some resource name suffixes.')
param env string = 'dev'

@description('Prefix for resource naming. Should be short and globally unique-ish.')
param namePrefix string = 'rib'

@description('Front Door (Standard/Premium) SKU name. Allowed: Standard_AzureFrontDoor, Premium_AzureFrontDoor')
@allowed([
  'Standard_AzureFrontDoor'
  'Premium_AzureFrontDoor'
])
param afdSku string = 'Standard_AzureFrontDoor'

@description('Origin public hostname or IP (backend ingress / public LB).')
param originHost string

@description('Origin host header to send (typically the ingress host / app domain).')
param originHostHeader string

@description('Health probe path for origin group.')
param originProbePath string = '/healthz'

@description('Rate limiting: requests per 60s window (edge WAF). Set 0 to skip rule creation.')
@minValue(0)
param rateLimitPerMinute int = 120

@description('Rate limit window minutes (AFD supports 1 or 5).')
@allowed([
  1
  5
])
param rateLimitWindowMinutes int = 1

@description('Enable creation of WAF resources (set false while provider type not available to avoid deployment failure).')
param enableWaf bool = false

@description('Cheap mode: omit managed rule sets (rate limit only) to reduce cost.')
param cheapMode bool = true

@description('AFD endpoint hostname prefix (will end with .azurefd.net if custom domain not mapped).')
param endpointName string = '${namePrefix}-${env}-afd'

@description('Enable WAF policy in Prevention mode (if false only Detection).')
param wafPrevention bool = true

var wafPolicyName = '${namePrefix}-${env}-waf'
var securityPolicyName = '${namePrefix}-${env}-secpol'
var originGroupName = '${namePrefix}-${env}-og'
var originName = '${namePrefix}-${env}-origin'
var routeName = '${namePrefix}-${env}-route'
// securityPolicyName reserved for future security policy binding (unused yet)

// Front Door Profile
resource profile 'Microsoft.Cdn/profiles@2024-02-01' = {
  name: endpointName
  location: 'global'
  sku: {
    name: afdSku
  }
}

// WAF policy (correct resource type for AFD Standard/Premium) - global scope
var managedRuleSets = cheapMode ? [] : [
  {
    ruleSetType: 'DefaultRuleSet'
    ruleSetVersion: '2.1'
  }
]

resource wafPolicy 'Microsoft.Cdn/cdnWebApplicationFirewallPolicies@2024-02-01' = if (enableWaf && rateLimitPerMinute > 0) {
  name: wafPolicyName
  location: 'Global'
  sku: {
    name: afdSku
  }
  properties: {
    policySettings: {
      enabledState: 'Enabled'
      mode: wafPrevention ? 'Prevention' : 'Detection'
      defaultCustomBlockResponseStatusCode: 429
    }
    managedRules: {
      managedRuleSets: managedRuleSets
    }
    rateLimitRules: {
      rules: [
        {
          name: 'RateLimitAll'
          priority: 1
          action: 'Block'
          enabledState: (enableWaf && rateLimitPerMinute > 0) ? 'Enabled' : 'Disabled'
          rateLimitThreshold: rateLimitPerMinute
          rateLimitDurationInMinutes: rateLimitWindowMinutes
          matchConditions: [
            {
              matchVariable: 'RemoteAddr'
              operator: 'IPMatch'
              matchValue: [ '0.0.0.0/0', '::/0' ]
            }
          ]
        }
      ]
    }
  }
}

// Security policy binding WAF to endpoint default domain
resource securityPolicy 'Microsoft.Cdn/profiles/securityPolicies@2024-02-01' = if (enableWaf && rateLimitPerMinute > 0) {
  name: securityPolicyName
  parent: profile
  properties: {
    parameters: {
      type: 'WebApplicationFirewall'
      wafPolicy: {
        id: wafPolicy.id
      }
      associations: [
        {
          domains: [
            { id: endpoint.id }
          ]
          patternsToMatch: [ '/*' ]
        }
      ]
    }
  }
  // implicit dependency via reference to wafPolicy.id
}

// Origin Group
resource originGroup 'Microsoft.Cdn/profiles/originGroups@2024-02-01' = {
  name: originGroupName
  parent: profile
  properties: {
    sessionAffinityState: 'Disabled'
    healthProbeSettings: {
      probePath: originProbePath
      probeRequestType: 'GET'
      probeProtocol: 'Http'
      probeIntervalInSeconds: 30
    }
    loadBalancingSettings: {
      sampleSize: 4
      successfulSamplesRequired: 3
      additionalLatencyInMilliseconds: 0
    }
  }
}

// Origin inside the group
resource originRes 'Microsoft.Cdn/profiles/originGroups/origins@2024-02-01' = {
  name: originName
  parent: originGroup
  properties: {
    hostName: originHost
    originHostHeader: originHostHeader
    httpPort: 80
    httpsPort: 443
    priority: 1
    weight: 1000
    enabledState: 'Enabled'
  }
}

// Endpoint (AFD endpoint)
resource endpoint 'Microsoft.Cdn/profiles/afdEndpoints@2024-02-01' = {
  name: endpointName
  parent: profile
  location: 'global'
  properties: {
    enabledState: 'Enabled'
  }
}

// Route mapping endpoint -> origin group
resource route 'Microsoft.Cdn/profiles/afdEndpoints/routes@2024-02-01' = {
  name: routeName
  parent: endpoint
  properties: {
    httpsRedirect: 'Enabled'
    originGroup: {
      id: originGroup.id
    }
    // Accept all hostnames on the endpoint until custom domain added
  supportedProtocols: [ 'Http', 'Https' ]
    linkToDefaultDomain: 'Enabled'
    patternsToMatch: [ '/*' ]
    forwardingProtocol: 'MatchRequest'
    enabledState: 'Enabled'
  }
  dependsOn: [ originRes ]
}

// TODO: Security policy binding once WAF policy path confirmed in region.

output endpointHost string = '${endpoint.name}.azurefd.net'
output wafPolicyResourceId string = (enableWaf && rateLimitPerMinute > 0) ? wafPolicy.id : ''
output securityPolicyId string = (enableWaf && rateLimitPerMinute > 0) ? securityPolicy.id : ''
output wafEnabledEffective bool = enableWaf && rateLimitPerMinute > 0
