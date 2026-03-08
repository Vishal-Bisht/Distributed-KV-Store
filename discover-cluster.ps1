# Discover and configure cluster nodes
# Run this on each machine to auto-configure cluster.json

param(
    [int]$NodeId = 0,  # Your node ID (1, 2, or 3). Set to 0 for auto-assign
    [string]$ScanRange = "",  # e.g., "192.168.1.100-110" or leave empty for auto
    [int[]]$Ports = @(8001, 8002, 8003)
)

# Get local IP
function Get-LocalIP {
    $ip = (Get-NetIPConfiguration | 
           Where-Object { $_.IPv4DefaultGateway -ne $null } | 
           Select-Object -First 1).IPv4Address.IPAddress
    return $ip
}

# Quick port check
function Test-Port {
    param([string]$IP, [int]$Port, [int]$Timeout = 100)
    try {
        $tcp = New-Object System.Net.Sockets.TcpClient
        $result = $tcp.BeginConnect($IP, $Port, $null, $null)
        $success = $result.AsyncWaitHandle.WaitOne($Timeout)
        $tcp.Close()
        return $success
    } catch {
        return $false
    }
}

$localIP = Get-LocalIP
Write-Host "Your IP: $localIP" -ForegroundColor Cyan

# Determine scan range
if (-not $ScanRange) {
    $parts = $localIP -split '\.'
    $subnet = "$($parts[0]).$($parts[1]).$($parts[2])"
    $ScanRange = "$subnet.1-254"
    Write-Host "Scanning subnet: $ScanRange" -ForegroundColor Yellow
}

# Parse range
$rangeParts = $ScanRange -split '\.'
$lastOctet = $rangeParts[3]
$baseIP = "$($rangeParts[0]).$($rangeParts[1]).$($rangeParts[2])"

if ($lastOctet -match '(\d+)-(\d+)') {
    $start = [int]$Matches[1]
    $end = [int]$Matches[2]
} else {
    $start = [int]$lastOctet
    $end = $start
}

Write-Host "Scanning for KV nodes on ports $($Ports -join ', ')..." -ForegroundColor Yellow
Write-Host ""

$foundNodes = @()

# Scan for nodes
for ($i = $start; $i -le $end; $i++) {
    $ip = "$baseIP.$i"
    foreach ($port in $Ports) {
        if (Test-Port -IP $ip -Port $port) {
            $nodeId = $port - 8000  # Derive node ID from port
            Write-Host "  Found node $nodeId at ${ip}:${port}" -ForegroundColor Green
            $foundNodes += @{
                id = $nodeId
                host = $ip
                port = $port
                http_port = 9000 + $nodeId
            }
        }
    }
    
    # Progress indicator
    if ($i % 10 -eq 0) {
        Write-Host "." -NoNewline
    }
}

Write-Host ""
Write-Host ""

# Add local node if not found
$localPort = if ($NodeId -gt 0) { 8000 + $NodeId } else { 8001 }
$localFound = $foundNodes | Where-Object { $_.host -eq $localIP }
if (-not $localFound) {
    if ($NodeId -eq 0) {
        $usedIds = $foundNodes | ForEach-Object { $_.id }
        for ($id = 1; $id -le 10; $id++) {
            if ($id -notin $usedIds) {
                $NodeId = $id
                break
            }
        }
    }
    Write-Host "Adding local node as Node $NodeId" -ForegroundColor Cyan
    $foundNodes += @{
        id = $NodeId
        host = "auto"
        port = 8000 + $NodeId
        http_port = 9000 + $NodeId
    }
}

# Sort by ID
$foundNodes = $foundNodes | Sort-Object { $_.id }

# Build config
$config = @{
    nodes = $foundNodes
    comment = "Auto-generated cluster config. 'auto' = local IP"
}

# Save config
$configPath = Join-Path $PSScriptRoot "cluster.json"
$config | ConvertTo-Json -Depth 3 | Set-Content $configPath

Write-Host "Configuration saved to cluster.json:" -ForegroundColor Green
Get-Content $configPath | Write-Host

Write-Host ""
Write-Host "To start your node, run:" -ForegroundColor Yellow
Write-Host "  .\start-cluster-node.ps1 -NodeId $NodeId"
