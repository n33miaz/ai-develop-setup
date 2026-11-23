$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'start-web-terminal.ps1')

function Assert-StartRefused {
    param([string] $Message)

    try {
        Start-WebTerminal -Config $PSCommandPath -Pull
    } catch {
        if ($_.Exception.Message -like $Message) { return }
        throw
    }
    throw "Expected startup to fail with: $Message"
}

$existingServices = @()
$pullCalls = 0

function Get-WebTerminalProcesses { $script:existingServices }
function git {
    if (($args -join ' ') -ne 'pull --ff-only') { throw 'Unexpected Git command.' }
    $script:pullCalls++
    Set-Variable -Name LASTEXITCODE -Value 1 -Scope 1
}
function cargo { throw 'Startup must stop before compilation.' }
function Start-Process { throw 'Startup must stop before starting services.' }

foreach ($name in 'ads-ptyhost', 'ads-daemon') {
    $existingServices = @([pscustomobject]@{ ProcessName = $name; Id = 123 })
    Assert-StartRefused "*Close the existing host or daemon windows*Running: $name (PID 123)*"
}
$existingServices = @()
if ($pullCalls -ne 0) { throw 'Git ran while a previous instance was still open.' }
Write-Output 'Second invocation refused for the host and daemon left by a previous run.'

$occupied = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 5173)
try {
    $occupied.ExclusiveAddressUse = $true
    $occupied.Start()
    Assert-StartRefused '*Port 5173 is unavailable. Close the Vite window*'
} finally {
    $occupied.Stop()
}
if ($pullCalls -ne 0) { throw 'Git ran despite the occupied port.' }
Write-Output 'Occupied port refused before pull or build.'

Assert-StartRefused '*git pull failed. Resolve the Git error*'
if ($pullCalls -ne 1) { throw 'Expected exactly one pull attempt.' }
Write-Output 'Failed pull stopped startup before build.'

$fixture = Join-Path (Split-Path -Parent $PSScriptRoot) '.test-tmp'
$null = [IO.Directory]::CreateDirectory($fixture)
$configPath = Join-Path $fixture ("web-terminal-" + [Guid]::NewGuid() + ".toml")
function Assert-ConfigOutcome {
    param([string] $Toml, [string] $Message)
    [IO.File]::WriteAllText($configPath, $Toml)
    try {
        Start-WebTerminal -Config $configPath
    } catch {
        if ($_.Exception.Message -like $Message) { return }
        throw
    }
    throw "Expected startup to stop with: $Message"
}

foreach ($origin in 'http://127.0.0.1:5174', 'http://localhost:5173', 'http://[::1]:5173') {
    Assert-ConfigOutcome "'ui_base_url' = '$origin'" '*ui_base_url*http://127.0.0.1:5173*'
}
foreach ($toml in '', "ui_base_url = 'http://127.0.0.1:5173/'", "[env]`nui_base_url = 'other'") {
    Assert-ConfigOutcome $toml 'Startup must stop before compilation.'
}
Write-Output 'Only the effective Vite origin reaches the build step.'
