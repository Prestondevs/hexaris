[CmdletBinding()]
param([switch]$Offline)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$repo = Split-Path -Parent $PSScriptRoot
$targetTriple = 'x86_64-pc-windows-msvc'
$network = @()
if ($Offline) { $network += '--offline' }

Push-Location $repo
try {
    cargo build --workspace --release --locked --target $targetTriple @network
    $metadata = cargo metadata --format-version 1 --locked --filter-platform $targetTriple @network | ConvertFrom-Json
    $tree = cargo tree --locked --workspace --target $targetTriple --edges normal --prefix none --format '{p}' @network
    $resolved = @{}
    foreach ($row in $tree) {
        if ($row -match '^(\S+) v(\S+)') { $resolved["$($Matches[1])@$($Matches[2])"] = $true }
    }

    $packages = $metadata.packages | Where-Object { $_.source -and $resolved.ContainsKey("$($_.name)@$($_.version)") } | Sort-Object name, version
    $app = $metadata.packages | Where-Object { $_.name -eq 'hexaris-desktop' -and !$_.source }
    $bin = Join-Path $metadata.target_directory "$targetTriple/release"
    $folder = Join-Path $repo 'dist'
    New-Item -ItemType Directory -Path $folder -Force | Out-Null
    $path = Join-Path $folder "hexaris-$($app.version)-windows-x64.zip"
    $staging = Join-Path $folder ".hexaris-$([Guid]::NewGuid().ToString('N')).zip"
    try {
        $file = [IO.File]::Open($staging, [IO.FileMode]::CreateNew)
        try {
            $zip = [IO.Compression.ZipArchive]::new($file, [IO.Compression.ZipArchiveMode]::Create, $true)
            try {
                $timestamp = [DateTimeOffset]::Parse('1980-01-01T00:00:00Z')
                function Add-ZipFile($source, $name) {
                    $entry = $zip.CreateEntry($name, [IO.Compression.CompressionLevel]::Optimal)
                    $entry.LastWriteTime = $timestamp
                    $sourceStream = [IO.File]::OpenRead($source)
                    try {
                        $destination = $entry.Open()
                        try { $sourceStream.CopyTo($destination) } finally { $destination.Dispose() }
                    } finally { $sourceStream.Dispose() }
                }
                Add-ZipFile (Join-Path $bin 'hexaris.exe') 'hexaris.exe'
                Add-ZipFile (Join-Path $bin 'hexaris-cli.exe') 'hexaris-cli.exe'
                foreach ($name in 'LICENSE', 'README.md', 'THIRD_PARTY_NOTICES.md', 'CONTRIBUTING.md', 'SECURITY.md', 'assets/ASSET_MANIFEST.toml') {
                    Add-ZipFile (Join-Path $repo $name) $name
                }
                foreach ($directory in 'assets/branding', 'assets/icons', 'assets/LICENSES', 'docs', 'tests/fixtures', 'scripts') {
                    foreach ($item in Get-ChildItem -LiteralPath (Join-Path $repo $directory) -File -Recurse | Sort-Object FullName) {
                        $name = [IO.Path]::GetRelativePath($repo, $item.FullName).Replace('\', '/')
                        Add-ZipFile $item.FullName $name
                    }
                }
                $lines = @('Package | License expression | Upstream')
                foreach ($package in $packages) {
                    $packageRoot = Split-Path -Parent $package.manifest_path
                    $entries = Get-ChildItem -LiteralPath $packageRoot -File | Where-Object Name -Match '^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT|AUTHORS)' | Sort-Object Name
                    foreach ($notice in $entries) {
                        Add-ZipFile $notice.FullName "licenses/$($package.name)-$($package.version)/$($notice.Name)"
                    }
                    $lines += "$($package.name) $($package.version) | $($package.license) | $($package.repository)"
                }
                $entry = $zip.CreateEntry('licenses/DEPENDENCIES.txt')
                $entry.LastWriteTime = $timestamp
                $writer = [IO.StreamWriter]::new($entry.Open())
                try { $writer.Write(($lines -join "`n") + "`n") } finally { $writer.Dispose() }
            } finally { $zip.Dispose() }
        } finally { $file.Dispose() }
        [IO.File]::Move($staging, $path, $true)
    } finally {
        if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging }
    }
    Get-FileHash -LiteralPath $path -Algorithm SHA256
} finally {
    Pop-Location
}
