# Copies the licensed PNG and OGG files into a build scrambled, so they are never distributed as standalone files.
# src/packed_assets.rs reverses this at load time. Run with no arguments as a Trunk post_build hook.

param(
    [string]$Source = (Join-Path $env:TRUNK_SOURCE_DIR 'assets/external'),
    [string]$Target = (Join-Path $env:TRUNK_STAGING_DIR 'assets/external')
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
public static class Scrambler
{
    // Must match SEED in src/packed_assets.rs.
    const uint Seed = 0x5EEDAE7F;

    public static void Apply(byte[] bytes)
    {
        uint state = Seed;
        for (int i = 0; i < bytes.Length; i++)
        {
            state = unchecked(state * 1664525u + 1013904223u);
            bytes[i] ^= (byte)(state >> 24);
        }
    }
}
'@

$files = Get-ChildItem $Source -Recurse -File | Where-Object Extension -in '.png', '.ogg'
foreach ($file in $files) {
    $relative = [IO.Path]::GetRelativePath($Source, $file.FullName)
    $out = Join-Path $Target $relative
    New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
    $bytes = [IO.File]::ReadAllBytes($file.FullName)
    [Scrambler]::Apply($bytes)
    [IO.File]::WriteAllBytes($out, $bytes)
}
Write-Host "Packed $($files.Count) licensed files into $Target"
