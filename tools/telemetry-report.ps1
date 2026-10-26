# Summarises the balancing logs the game writes beside its saves (telemetry\slotN_RUN.jsonl, one per playthrough): the
# hero's level on first entering each zone and on beating each encounter, then per-zone damage by element and attack
# kind, and Aether uptime, split by the commit each build came from.
param(
    [string]$Path = (Join-Path $env:APPDATA 'Aetherfall\rs\telemetry'),
    [int]$Slot = 0,
    [string]$Commit = ''
)

$files = if (Test-Path $Path -PathType Leaf) { @(Get-Item $Path) } else { @(Get-ChildItem $Path -Filter *.jsonl -ErrorAction SilentlyContinue) }
if ($Slot -gt 0) { $files = $files | Where-Object Name -like "slot$($Slot)_*" }
if (-not $files) { Write-Host "No logs at $Path yet."; return }
$events = foreach ($file in $files) {
    Get-Content $file.FullName | Where-Object { $_.Trim() } | ForEach-Object {
        $e = $_ | ConvertFrom-Json
        $e | Add-Member -NotePropertyName run -NotePropertyValue $file.BaseName -PassThru
    }
}
if ($Commit) { $events = $events | Where-Object { $_.commit -like "$Commit*" } }

$notes = $events | Where-Object event -eq 'note'
if ($notes) {
    Write-Host "`n== Notes"
    $notes | Sort-Object t | Format-Table run, @{ n = 'time'; e = { [TimeSpan]::FromSeconds($_.t).ToString('h\:mm\:ss') } }, level, text -AutoSize -Wrap
}

Write-Host "`n== First entry into each zone"
$events | Where-Object event -eq 'zone_enter' | Sort-Object t |
    Format-Table run, commit, @{ n = 'time'; e = { [TimeSpan]::FromSeconds($_.t).ToString('h\:mm\:ss') } }, map, area_level,
    @{ n = 'hero'; e = { $_.level } }, @{ n = 'over'; e = { $_.level - $_.area_level } } -AutoSize

Write-Host "== Encounters beaten"
$events | Where-Object event -eq 'encounter' | Sort-Object t |
    Format-Table run, commit, kind, id, map, @{ n = 'hero'; e = { $_.level } }, foe_level, area_level, seconds, @{ n = 'mods'; e = { $_.mods -join ', ' } } -AutoSize

Write-Host "== Zones: time, XP and gold gained, damage by element and Aether uptime (all visits)"
$events | Where-Object event -eq 'zone_summary' | Group-Object map, commit | ForEach-Object {
    $damage = @{}; $kinds = @{}; $seconds = 0; $aether = 0; $kills = 0; $deaths = 0; $exp = 0; $gold = 0
    foreach ($v in $_.Group) {
        $seconds += $v.seconds; $aether += $v.aether_seconds; $kills += $v.kills; $deaths += $v.deaths
        $exp += $v.exp_gained; $gold += $v.gold_gained
        foreach ($p in $v.damage.PSObject.Properties) { $damage[$p.Name] += $p.Value }
        foreach ($p in $v.damage_by_kind.PSObject.Properties) { $kinds[$p.Name] += $p.Value }
    }
    $total = ($damage.Values | Measure-Object -Sum).Sum
    $share = { param($table) ($table.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object { '{0} {1:P0}' -f $_.Key, ($_.Value / [Math]::Max(1, $total)) }) -join ', ' }
    [pscustomobject]@{
        map     = $_.Group[0].map
        commit  = $_.Group[0].commit
        minutes = [Math]::Round($seconds / 60, 1)
        kills   = $kills
        deaths  = $deaths
        exp     = $exp
        gold    = $gold
        "exp/min" = [Math]::Round($exp / [Math]::Max(1, $seconds / 60))
        aether  = '{0:P0}' -f ($aether / [Math]::Max(1, $seconds))
        damage  = [Math]::Round($total)
        element = & $share $damage
        kind    = & $share $kinds
    }
} | Format-Table -AutoSize -Wrap
