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
Invoke-Check pnpm @('types:check')
Invoke-Check pnpm @('lint')
Invoke-Check pnpm @('typecheck')
Invoke-Check pnpm @('test')
Invoke-Check pnpm @('build')
Invoke-Check pnpm @('test:e2e')
Invoke-Check pnpm @('docs:check')
