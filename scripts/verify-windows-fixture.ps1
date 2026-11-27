[CmdletBinding()]
param([string]$Cli = 'target/release/hexaris-cli.exe')

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    $tool = (Resolve-Path -LiteralPath $Cli).Path
    $source = (Resolve-Path -LiteralPath 'tests/fixtures/score.c').Path
    $directory = (New-Item -ItemType Directory -Path ".dev/validation-$([Guid]::NewGuid().ToString('N'))").FullName
    Push-Location $directory
    try {
        cl.exe /nologo /W4 /WX /Od /GS- /TC $source /Foscore.obj /link /nologo /nodefaultlib /entry:entry /subsystem:console /map:score.map /out:score.exe
        $pattern = '^\s+\w+:\w+\s+score\s+([0-9A-Fa-f]+)\s+f\s'
        $found = Select-String -LiteralPath 'score.map' -Pattern $pattern
        if (@($found).Count -ne 1) { throw 'Could not resolve score from the fixture link map' }
        $symbolHex = $found.Matches[0].Groups[1].Value.TrimStart('0').ToUpperInvariant()
        $symbol = [Convert]::ToUInt64($symbolHex, 16)

        $metadata = & $tool inspect score.exe | ConvertFrom-Json
        if ($metadata.format -ne 'PE' -or $metadata.architecture -ne 'X86_64') { throw 'Unexpected fixture format or architecture' }
        $analysis = & $tool analyze score.exe | ConvertFrom-Json -AsHashtable
        $score = $analysis.functions["$symbol"]
        $entry = $analysis.functions["$($metadata.entry)"]
        if ($score.instructions.Count -ne 9 -or $score.blocks.Count -ne 4) { throw 'Unexpected instruction or block count for score' }
        $offsets = @($score.blocks | ForEach-Object { [uint64]$_.address - $symbol })
        if (($offsets -join ',') -ne '0,11,20,26') { throw 'Unexpected basic block offsets' }
        if (!($entry.references | Where-Object { $_.kind -eq 'Call' -and $_.target -eq $symbol })) { throw 'The fixture call to score was not discovered' }

        $decompilation = & $tool decompile score.exe "0x$symbolHex" | ConvertFrom-Json
        if (!$decompilation.code) { throw "No pseudocode for score. $($decompilation.unavailable)" }
        $body = $decompilation.code
        $harness = @"
#include <stdint.h>
extern unsigned score(unsigned);
$body

int main(void) {
    static const uint32_t boundaries[] = {0u, 1u, 9u, 10u, 12u, 0x7fffffffu, 0x80000000u, 0xffffffffu};
    for (unsigned i = 0; i < sizeof(boundaries) / sizeof(boundaries[0]); ++i) {
        uint32_t value = boundaries[i];
        if (score(value) != sub_$symbolHex(value)) return 1;
    }
    uint32_t value = 1;
    for (unsigned i = 0; i < 250000u; ++i) {
        value = value * 1664525u + 1013904223u;
        if (score(value) != sub_$symbolHex(value)) return 2;
    }
    return 0;
}
"@
        Set-Content -LiteralPath 'compare.c' -Value $harness -Encoding ascii
        cl.exe /nologo /W4 /WX /O2 /TC compare.c $source /Fecompare.exe
        & .\compare.exe

        & $tool project create score.exe score.hxproj
        & $tool project annotate score.hxproj "0x$symbolHex" --name score --comment 'unsigned arithmetic fixture'
        $annotation = & $tool project annotation score.hxproj "0x$symbolHex" | ConvertFrom-Json
        if ($annotation.name -ne 'score' -or $annotation.comment -ne 'unsigned arithmetic fixture') { throw 'Annotation round trip failed' }
        $listing = & $tool disassemble score.hxproj "0x$symbolHex" --count 2 | ConvertFrom-Json
        if ($listing.instructions.Count -ne 2 -or $listing.next_address -ne ($symbol + 9)) { throw 'Sequential decode did not advance to the branch' }

        $hash = (Get-FileHash -LiteralPath 'score.exe').Hash
        $call = $entry.references | Where-Object { $_.kind -eq 'Call' -and $_.target -eq $symbol } | Select-Object -First 1
        $callOffset = $entry.instructions | Where-Object address -EQ $call.source | Select-Object -ExpandProperty file_offset
        & $tool patch score.exe patched.exe "$callOffset" --expect 'E8' --replace '90'
        if ((Get-FileHash -LiteralPath 'score.exe').Hash -ne $hash -or (Get-FileHash -LiteralPath 'patched.exe').Hash -eq $hash) { throw 'Patch copy validation failed' }
        Write-Output 'Verified PE load, CFG, direct call, linear decode, project round trip, guarded patch, and 250,008 decompilation comparisons.'
    } finally {
        Pop-Location
    }
} finally {
    Pop-Location
}
