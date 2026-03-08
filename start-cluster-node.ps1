# Auto-discovery cluster node starter
# Usage: .\start-cluster-node.ps1 -NodeId 1
# Edit cluster.json to configure your cluster

param(
    [Parameter(Mandatory=$true)]
    [int]$NodeId
)

# Get local IP address (prefers non-loopback IPv4)
function Get-LocalIP {
    # First try: get IP from adapter with default gateway (most reliable)
    $ip = (Get-NetIPConfiguration | 
           Where-Object { $_.IPv4DefaultGateway -ne $null } | 
           Select-Object -First 1).IPv4Address.IPAddress
    
    if (-not $ip) {
        # Fallback: get any non-loopback, non-link-local IPv4
        $ip = Get-NetIPAddress -AddressFamily IPv4 | 
              Where-Object { 
                  $_.InterfaceAlias -notmatch 'Loopback' -and 
                  $_.IPAddress -ne '127.0.0.1' -and 
                  $_.IPAddress -notlike '169.254.*'  # Exclude link-local
              } |
              Select-Object -First 1 -ExpandProperty IPAddress
    }
    
    if (-not $ip) {
        Write-Error "Could not auto-detect local IP address"
        exit 1
    }
    
    return $ip
}

# Load cluster configuration
$configPath = Join-Path $PSScriptRoot "cluster.json"
if (-not (Test-Path $configPath)) {
    Write-Error "cluster.json not found. Please create it with node configurations."
    exit 1
}

$config = Get-Content $configPath | ConvertFrom-Json
$localIP = Get-LocalIP

Write-Host "Detected local IP: $localIP" -ForegroundColor Cyan

# Find this node's config
$thisNode = $config.nodes | Where-Object { $_.id -eq $NodeId }
if (-not $thisNode) {
    Write-Error "Node ID $NodeId not found in cluster.json"
    exit 1
}

# Resolve 'auto' to local IP
$thisHost = if ($thisNode.host -eq "auto") { $localIP } else { $thisNode.host }
$thisAddr = "${thisHost}:$($thisNode.port)"

# Build peer list (all nodes except this one)
$peers = @()
foreach ($node in $config.nodes) {
    if ($node.id -ne $NodeId) {
        $peerHost = if ($node.host -eq "auto") { $localIP } else { $node.host }
        $peers += "${peerHost}:$($node.port)"
    }
}
$peerList = $peers -join ","

Write-Host ""
Write-Host "Starting Node $NodeId" -ForegroundColor Green
Write-Host "  Address: $thisAddr"
Write-Host "  HTTP API: http://${thisHost}:$($thisNode.http_port)"
Write-Host "  Peers: $peerList"
Write-Host ""

# Start the node
$exePath = Join-Path $PSScriptRoot "target\release\kv.exe"
if (-not (Test-Path $exePath)) {
    $exePath = Join-Path $PSScriptRoot "target\debug\kv.exe"
}

& $exePath -i $NodeId -a $thisAddr -p $peerList
