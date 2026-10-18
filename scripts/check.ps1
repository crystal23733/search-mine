$ErrorActionPreference = 'Stop'

function Invoke-Check {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Check failed: $Program $($Arguments -join ' ')" }
}

Invoke-Check cargo @('fmt', '--all', '--', '--check')
Invoke-Check cargo @('clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings')
Invoke-Check cargo @('test', '--locked', '--workspace')
Invoke-Check cargo @('check', '--locked', '-p', 'liar-wasm', '--target', 'wasm32-unknown-unknown')
Invoke-Check yarn @('types:check')
Invoke-Check yarn @('wasm:build')
Invoke-Check yarn @('fixtures:check')
Invoke-Check yarn @('tokens:check')
Invoke-Check yarn @('format:check')
Invoke-Check yarn @('lint')
Invoke-Check yarn @('typecheck')
Invoke-Check yarn @('test')
Invoke-Check yarn @('test:worker-probe')
Invoke-Check yarn @('test:native-worker-trace')
Invoke-Check yarn @('build')
Invoke-Check yarn @('test:e2e')
Invoke-Check yarn @('docs:check')
