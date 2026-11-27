[CmdletBinding()]
param(
    [string]$Cli = 'target/release/hexaris-cli.exe',
    [string]$GhidraXml
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    $tool = (Resolve-Path -LiteralPath $Cli).Path
    $source = (Resolve-Path -LiteralPath 'tests/fixtures/unwind.c').Path
    $scoreSource = (Resolve-Path -LiteralPath 'tests/fixtures/score.c').Path
    if ($GhidraXml) { $GhidraXml = (Resolve-Path -LiteralPath $GhidraXml).Path }
    $directory = (New-Item -ItemType Directory -Path ".dev/workstation-$([Guid]::NewGuid().ToString('N'))").FullName
    Push-Location $directory
    try {
        cl.exe /nologo /W4 /WX /Od /GS- /TC $source /Founwind.obj /link /nologo /nodefaultlib /entry:entry /subsystem:console /opt:noref /map:unwind.map /out:unwind.exe
        cl.exe /nologo /W4 /WX /Od /GS- /TC $scoreSource /Foscore.obj /link /nologo /nodefaultlib /entry:entry /subsystem:console /out:score.exe

        $hiddenMatch = Select-String -LiteralPath 'unwind.map' -Pattern '^\s+\w+:\w+\s+hidden_unwind\s+([0-9A-Fa-f]+)\s+f\s'
        if (@($hiddenMatch).Count -ne 1) { throw 'Missing hidden_unwind in the linker map' }
        $hidden = [Convert]::ToUInt64($hiddenMatch.Matches[0].Groups[1].Value, 16)
        $metadata = & $tool inspect unwind.exe | ConvertFrom-Json
        $unwind = $metadata.unwind_functions | Where-Object address -EQ $hidden
        if (!$unwind -or $unwind.end -le $hidden) { throw 'Missing validated hidden function range' }
        $export = $metadata.exports | Where-Object name -EQ 'exported_leaf'
        if (!$export) { throw 'Missing fixture export' }
        $analysis = & $tool analyze unwind.exe | ConvertFrom-Json -AsHashtable
        if (!$analysis.functions["$hidden"] -or $analysis.functions["$($export.address)"].name -ne 'exported_leaf') {
            throw 'Unwind or export seeding failed'
        }
        $references = & $tool references unwind.exe ('0x{0:X}' -f $hidden) | ConvertFrom-Json
        if ($references.callers.Count -ne 0 -or $references.callees.Count -ne 1) { throw 'Incorrect direct call index for the unreachable fixture function' }

        $PSNativeCommandUseErrorActionPreference = $false
        try {
            $output = & $tool triage unwind.exe missing.exe score.exe 2>triage-error.log
            $status = $LASTEXITCODE
        } finally { $PSNativeCommandUseErrorActionPreference = $true }
        $batch = $output | ConvertFrom-Json
        if ($status -ne 1 -or $batch.failed -ne 1 -or $batch.reports.Count -ne 3 -or !$batch.reports[1].error) { throw 'Batch failure reporting lost a record' }
        $digest = (Get-FileHash -LiteralPath 'score.exe' -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($batch.reports[2].report.sha256 -ne $digest -or !$batch.reports[2].report.samples) { throw 'Triage report disagrees with file checksum' }

        & $tool project create score.exe score.hxproj
        & $tool project annotate score.hxproj 0x140001000 --bookmark 'before import'
        if ($GhidraXml) {
            $before = (Get-FileHash -LiteralPath 'score.hxproj').Hash
            $preview = & $tool project import-ghidra score.hxproj $GhidraXml --dry-run | ConvertFrom-Json
            if ($preview.function_names -lt 1 -or $preview.comments -lt 1 -or (Get-FileHash -LiteralPath 'score.hxproj').Hash -ne $before) { throw 'Ghidra preview failed or wrote the project' }
            $merge = & $tool project import-ghidra score.hxproj $GhidraXml | ConvertFrom-Json
            if ($merge.existing -lt 1 -or $merge.comments -ne $preview.comments) { throw 'Ghidra merge did not preserve a local bookmark' }
            $annotation = & $tool project annotation score.hxproj 0x140001000 | ConvertFrom-Json
            if ($annotation.name -ne 'fixture_score' -or $annotation.comment -ne 'Unsigned comparison & return' -or $annotation.bookmark -ne 'before import') { throw 'Imported Ghidra annotations did not round-trip' }
        }
        $bookmarks = & $tool project bookmarks score.hxproj | ConvertFrom-Json -NoEnumerate
        if ($bookmarks[0].name -ne 'before import') { throw 'Bookmark not found after reopening project' }
        Write-Output 'Verified PE unwind and export roots, call indexing, batch triage, and portable bookmarks.'
        if ($GhidraXml) { Write-Output 'Verified preview and import of an actual Ghidra XML export while preserving existing annotations.' }
    } finally { Pop-Location }
} finally { Pop-Location }
