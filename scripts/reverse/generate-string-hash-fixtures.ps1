$ErrorActionPreference='Stop'
$root=$PWD.Path
$source='scripts/reverse/framework/StringHashOracle.cs'
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/optimize+','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/StringHashOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+=$source
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/string-hash-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'String hash oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/StringHashOracle-'+$arch+'.exe')) *> ('.cache/verification/string-hash-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'String hash oracle failed;local log'}
}
$x=Get-Content .cache/verification/string-hash-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/string-hash-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($source;'scripts/reverse/generate-string-hash-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='.NET Framework 4.8 String.GetHashCode and System.Random(hash).Next, no original service executed';x86=$x.cases;x64=$y.cases}|ConvertTo-Json -Depth 6 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-string-hash.json'),$result,[Text.UTF8Encoding]::new($false))
$diff=0;for($k=0;$k-lt$x.cases.Count;$k++){if($x.cases[$k].hash-ne$y.cases[$k].hash){$diff++}}
Write-Output ('String hash:'+$x.cases.Count+' cases per architecture;x86/x64 hash differences:'+$diff)
