<#
.SYNOPSIS
    Smoke-test managed backend reachability (same checks as agent-memory doctor prerequisites).

.NOTES
    Does not send credentials. Complements: py -3 -m agentic_memory.cli status --json
    from D:\code\agentic-memory after agent-memory login.
#>

$ErrorActionPreference = "Stop"
$base = "https://backend.agentmemorylabs.com"

$h = Invoke-RestMethod -Uri "$base/health" -TimeoutSec 60
if ($h.status -ne "ok") { throw "unexpected health: $($h | ConvertTo-Json -Compress)" }

$o = Invoke-RestMethod -Uri "$base/health/onboarding" -TimeoutSec 60
if ($o.status -ne "ok") { throw "unexpected onboarding: $($o | ConvertTo-Json -Compress)" }

Write-Host "OK: backend health + onboarding status=ok (deployment_mode=$($o.deployment_mode))"
