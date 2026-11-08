param(
    [Parameter(Mandatory = $true)][int]$ZoneProcessId,
    [ValidateRange(2, 60)][int]$DurationSeconds = 10
)

$ErrorActionPreference = 'Stop'
$zoneProcess = Get-Process -Id $ZoneProcessId
if ($zoneProcess.ProcessName -ne 'zone') {
    throw 'The specified process must be the Zone executable.'
}
$zoneRootStartTicks = $zoneProcess.StartTime.Ticks

function Assert-ZoneIdentity {
    $currentZoneRoot = Get-Process -Id $ZoneProcessId -ErrorAction SilentlyContinue
    if ($null -eq $currentZoneRoot -or $currentZoneRoot.ProcessName -ne 'zone' -or $currentZoneRoot.StartTime.Ticks -ne $zoneRootStartTicks) {
        throw 'Zone exited or its process ID was reused during sampling. No sample is reported.'
    }
}

function Get-ZoneProcessIds {
    $allProcesses = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
    $selected = [System.Collections.Generic.HashSet[int]]::new()
    [void]$selected.Add($ZoneProcessId)
    do {
        $added = $false
        foreach ($entry in $allProcesses) {
            if ($selected.Contains([int]$entry.ParentProcessId)) {
                $added = $selected.Add([int]$entry.ProcessId) -or $added
            }
        }
    } while ($added)
    return @($selected)
}

function Get-ZoneSnapshot {
    Assert-ZoneIdentity
    $snapshot = @{}
    foreach ($zoneId in (Get-ZoneProcessIds)) {
        $process = Get-Process -Id $zoneId -ErrorAction SilentlyContinue
        if ($null -ne $process) {
            $snapshot[$zoneId] = @{
                Name = $process.ProcessName
                StartTicks = $process.StartTime.Ticks
                CpuSeconds = $process.TotalProcessorTime.TotalSeconds
                WorkingSet = $process.WorkingSet64
                PrivateCommit = $process.PrivateMemorySize64
            }
        }
    }
    Assert-ZoneIdentity
    return $snapshot
}

$before = Get-ZoneSnapshot
$timer = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $DurationSeconds
$after = Get-ZoneSnapshot
$timer.Stop()
$cpuSeconds = 0.0
$workingSet = [long]0
$privateCommit = [long]0
$matched = 0
foreach ($zoneId in $after.Keys) {
    $entry = $after[$zoneId]
    $workingSet += $entry.WorkingSet
    $privateCommit += $entry.PrivateCommit
    if ($before.ContainsKey($zoneId) -and $before[$zoneId].StartTicks -eq $entry.StartTicks) {
        $cpuSeconds += [Math]::Max(0, $entry.CpuSeconds - $before[$zoneId].CpuSeconds)
        $matched += 1
    }
}

[pscustomobject]@{
    ObservedAtUtc = [DateTime]::UtcNow.ToString('o')
    ZoneProcessId = $ZoneProcessId
    DurationSeconds = [Math]::Round($timer.Elapsed.TotalSeconds, 3)
    LogicalProcessors = [Environment]::ProcessorCount
    CpuPercentOfMachine = [Math]::Round(100 * $cpuSeconds / $timer.Elapsed.TotalSeconds / [Environment]::ProcessorCount, 3)
    PrivateCommitMiB = [Math]::Round($privateCommit / 1MB, 2)
    SummedWorkingSetMiB = [Math]::Round($workingSet / 1MB, 2)
    ProcessCount = $after.Count
    MatchedCpuProcessCount = $matched
    Includes = 'Zone and its current WebView2 descendants; no system-wide browser processes'
    Caveat = 'Working sets include shared pages and are summed. CPU excludes processes born/exited during sampling and is normalized to the logical CPU count available to the sampler, which may be restricted by affinity or a job. Record build profile separately. This machine and sample only; not a comparative benchmark.'
} | ConvertTo-Json
