$ErrorActionPreference='Stop'
$root=$PWD.Path
$binary='../ServidorVindictus/Server/zh-tw-x-gm/Bin/PlayerService.dll'
$ilspy='./.tools/dotnet/ilspycmd.exe'
if((Get-Item -LiteralPath $binary).Length-gt16MB){throw 'Managed target exceeds admitted bound'}
$hash=(Get-FileHash -LiteralPath $binary).Hash.ToLowerInvariant()
if($hash-ne'f23d8b9df77c45dc332921cc3ccb22cdefaaa98703a863c69f6bf9f53e9c04f6'){throw 'PlayerService artifact changed'}
# Existing execution-free ILSpy fallback; no target Assembly.Load or service launch.
& $ilspy -il -t RouletteManager $binary *> .cache/framework/RouletteManager.il
if($LASTEXITCODE-ne0){throw 'Original IL recovery failed;local log'}
$il=[IO.File]::ReadAllText((Join-Path $root '.cache/framework/RouletteManager.il'))
$types=[ordered]@{}
foreach($name in @('RouletteManager','PlayerService.ItemServiceCore.MarbleSystem.MarbleManager')){
 $m=[regex]::Match($il,'(?m)^\.class [^\r\n]*\b'+[regex]::Escape($name)+'\r?$')
 if(-not$m.Success-or-not$m.Value.Contains('beforefieldinit')){throw 'Original static initialization flag changed'}
 $header=[Text.Encoding]::UTF8.GetBytes($m.Value.Trim())
 $types[$name]=[ordered]@{beforefieldinit=$true;header_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($header)).ToLowerInvariant()}
}
$assemblies=@{}
foreach($arch in @('x86','x64')){
 $framework=if($arch-eq'x86'){'Framework'}else{'Framework64'}
 $path=Join-Path $env:windir ('Microsoft.NET/'+$framework+'/v4.0.30319/mscorlib.dll')
 $assemblies[$arch]=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()
 & $ilspy -t System.Random $path *> ('.cache/framework/FrameworkRandom.'+$arch+'.cs')
 if($LASTEXITCODE-ne0){throw 'Framework Random type recovery failed;local log'}
}
$x=(Get-FileHash .cache/framework/FrameworkRandom.x86.cs).Hash.ToLowerInvariant()
$y=(Get-FileHash .cache/framework/FrameworkRandom.x64.cs).Hash.ToLowerInvariant()
if($x-ne$y){throw 'Framework Random type differs by architecture'}
$random=[IO.File]::ReadAllText((Join-Path $root '.cache/framework/FrameworkRandom.x86.cs'))
if(-not$random.Contains(': this(Environment.TickCount)')){throw 'Default Random seed source changed'}
$report=[ordered]@{player_service_sha256=$hash;inspector='existing ilspycmd -il;static analysis only,no target execution';types=$types;framework_random=[ordered]@{x86_assembly_sha256=$assemblies.x86;x64_assembly_sha256=$assemblies.x64;type_source_sha256=$x;x86_x64_type_source_equal=$true;default_seed='Environment.TickCount';migrated_in_this_step=$false}}
[IO.File]::WriteAllText((Join-Path $root '.cache/verification/initial-events-metadata.json'),($report|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
Write-Output 'Static event metadata PASS:two beforefieldinit types;Framework Random type x86=x64;no target executed.'
