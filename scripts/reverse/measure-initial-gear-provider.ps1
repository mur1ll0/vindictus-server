$ErrorActionPreference='Stop'
$root=$PWD.Path
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-',('/platform:'+$arch),('/out:.cache/framework/InitialGearProvider-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Data','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 [IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialGearProvider.cs'),[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/InitialGearProvider.cs.in')))
 $args+='.cache/framework/InitialGearProvider.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-gear-provider-build-'+$arch+'.log')
 if($LASTEXITCODE-ne0){throw 'Readonly provider probe compilation failed;local log'}
 $provider=[IO.Path]::GetFullPath((Join-Path $root ('../ServidorVindictus/Server/zh-tw-x-gm/Bin/'+$arch+'/System.Data.SQLite.DLL')))
 $database=[IO.Path]::GetFullPath((Join-Path $root '../ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'))
 $before=(Get-FileHash -LiteralPath $database).Hash
 & ('.cache/framework/InitialGearProvider-'+$arch+'.exe') $provider $database *> ('.cache/private-content/initial-gear-provider-'+$arch+'.json')
 if($LASTEXITCODE-ne0){throw 'Readonly provider probe failed;details private'}
 if((Get-FileHash -LiteralPath $database).Hash-cne$before){throw 'Original database changed during provider probe'}
}
$x=@(Get-Content .cache/private-content/initial-gear-provider-x86.json -Raw|ConvertFrom-Json -DateKind String)
$y=@(Get-Content .cache/private-content/initial-gear-provider-x64.json -Raw|ConvertFrom-Json -DateKind String)
if(($x|ConvertTo-Json -Depth 4 -Compress)-cne($y|ConvertTo-Json -Depth 4 -Compress)){throw 'Provider architecture divergence'}
if($x.Count-ne201-or@($x|Where-Object{$_.runtime_type-cne'System.Boolean'-or$_.string_branch}).Count-ne0){throw 'Unmeasured provider profile'}
$report=[ordered]@{verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');rows=$x.Count;true_rows=@($x|Where-Object loaded).Count;runtime_type='System.Boolean';GetString_throws=$true;framework_profiles=@('Framework48-x86','Framework48-x64');reference_unchanged=$true;legacy_service_executed=$false;raw_rows_private=$true;scope='owned readonly SQLiteConnection query;original HeroesContentsLoader BIT fallback branch'}
$report|ConvertTo-Json -Depth 5|Set-Content -LiteralPath docs/evidence/auth/initial-gear-provider.json -Encoding utf8
Write-Output ('Provider PASS;rows='+$x.Count+';x86=x64;Boolean fallback;source readonly;raw rows private.')
