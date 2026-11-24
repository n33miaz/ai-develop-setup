[CmdletBinding()]
param(
    [string] $Config,
    [switch] $Pull,
    [ValidateRange(1, 120)]
    [int] $ReadyTimeoutSeconds = 15
)

function Get-WebTerminalProcesses {
    Get-Process -Name ads-ptyhost, ads-daemon -ErrorAction SilentlyContinue
}

function Assert-WebTerminalAvailable {
    param([int] $Port)

    $running = @(Get-WebTerminalProcesses)
    if ($running.Count) {
        $services = ($running | ForEach-Object { "$($_.ProcessName) (PID $($_.Id))" }) -join ', '
        throw "Close the existing host or daemon windows before retrying. Running: $services."
    }

    $probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
    try {
        $probe.ExclusiveAddressUse = $true
        $probe.Start()
    } catch [Net.Sockets.SocketException] {
        throw "Port $Port is unavailable. Close the Vite window or the application using localhost:$Port before retrying."
    } finally {
        $probe.Stop()
    }
}

function Wait-WebTerminalService {
    param(
        [Diagnostics.Process] $Process,
        [string] $Name,
        [int] $TimeoutSeconds,
        [scriptblock] $Ready
    )

    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.Elapsed.TotalSeconds -lt $TimeoutSeconds -and !$Process.HasExited) {
        if ((& $Ready) -and !$Process.HasExited) { return }
        Start-Sleep -Milliseconds 100
    }
    if ($Process.HasExited) {
        throw "$Name exited before becoming ready with exit code $($Process.ExitCode). Check its console before retrying."
    }
    throw "$Name did not become ready. Check its console and close any remaining startup windows before retrying."
}

function Start-WebTerminal {
    param(
        [string] $Config,
        [switch] $Pull,
        [int] $ReadyTimeoutSeconds = 15
    )

    $ErrorActionPreference = 'Stop'
    $PSNativeCommandUseErrorActionPreference = $false
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this script from a regular PowerShell window. Close the Administrator window before retrying.'
    }

    $root = Split-Path -Parent $PSScriptRoot
    $port = 5173
    $mutex = [Threading.Mutex]::new($false, "Global\ads-web-terminal-$($identity.User.Value)")
    $locked = $false
    try {
        try {
            $locked = $mutex.WaitOne(0)
        } catch [Threading.AbandonedMutexException] {
            $locked = $true
        }
        if (!$locked) {
            throw 'Another web terminal launcher is running. Close its PowerShell window before retrying.'
        }

        Assert-WebTerminalAvailable -Port $port
        if (!$Config) { throw 'Pass -Config with the path to the daemon TOML (see crates/daemon/README.md).' }
        $configFile = Get-Item -LiteralPath $Config
        if ($configFile -isnot [IO.FileInfo]) { throw 'Pass a daemon TOML file to -Config.' }

        Push-Location -LiteralPath $root
        try {
            if ($Pull) {
                git pull --ff-only
                if ($LASTEXITCODE -ne 0) { throw 'git pull failed. Resolve the Git error before retrying.' }
            }

            $origin = "http://127.0.0.1:$port"
            python -I (Join-Path $PSScriptRoot 'check_web_terminal_origin.py') $configFile.FullName $origin
            if ($LASTEXITCODE -ne 0) {
                throw "Check the daemon TOML and set ui_base_url to $origin before retrying."
            }

            $ui = Join-Path $root 'ui'
            $vite = 'node_modules/vite/bin/vite.js'
            if (!(Test-Path -LiteralPath (Join-Path $ui $vite) -PathType Leaf)) {
                throw 'Install the UI dependencies with npm ci in ui before retrying.'
            }
            $node = (Get-Command node.exe -CommandType Application).Source
            $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
            $target = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($target)
            Assert-WebTerminalAvailable -Port $port
            cargo build -p ads-ptyhost -p ads-daemon -j 2 --target-dir $target
            if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed. Resolve the build error before retrying.' }

            Assert-WebTerminalAvailable -Port $port
            $hostProcess = Start-Process -FilePath (Join-Path $target 'debug/ads-ptyhost.exe') -PassThru
            Wait-WebTerminalService $hostProcess 'Terminal host' $ReadyTimeoutSeconds {
                $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', "ads-ptyhost.$($identity.User.Value)")
                try {
                    $pipe.Connect(100)
                    return $pipe.IsConnected
                } catch [TimeoutException] {
                    return $false
                } finally {
                    $pipe.Dispose()
                }
            }

            $viteProcess = Start-Process -FilePath $node -ArgumentList $vite -WorkingDirectory $ui -PassThru
            Wait-WebTerminalService $viteProcess 'Vite' $ReadyTimeoutSeconds {
                $listener = "^\s*TCP\s+127\.0\.0\.1:$port\s+0\.0\.0\.0:0\s+\S+\s+$($viteProcess.Id)\s*$"
                [bool](netstat.exe -ano -p tcp | Select-String -Pattern $listener -Quiet)
            }

            Write-Host 'Open the full link printed below. Keep the host, Vite, and daemon consoles open.'
            & (Join-Path $target 'debug/ads-daemon.exe') --config $configFile.FullName
            if ($LASTEXITCODE -ne 0) { throw 'The daemon exited with an error. Check its output before retrying.' }
        } finally {
            Pop-Location
        }
    } finally {
        if ($locked) { $mutex.ReleaseMutex() }
        $mutex.Dispose()
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    Start-WebTerminal @PSBoundParameters
}
