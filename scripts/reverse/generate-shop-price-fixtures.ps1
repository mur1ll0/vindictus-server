$ErrorActionPreference='Stop'
$root=$PWD.Path
$calc='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/PriceCalc.cs'
$template='scripts/reverse/framework/ShopPriceOracle.cs.in'
$s=[IO.File]::ReadAllText((Join-Path $root $calc));$i=$s.IndexOf('public static class PriceCalc');if($i-lt0){throw 'Source class missing'}
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__PRICECALC__',$s.Substring($i))
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ShopPriceOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/optimize+','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ShopPriceOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/ShopPriceOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/shop-price-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Shop price oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/ShopPriceOracle-'+$arch+'.exe')) *> ('.cache/verification/shop-price-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Shop price oracle failed;local log'}
}
$x=Get-Content .cache/verification/shop-price-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/shop-price-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($calc;$template;'scripts/reverse/generate-shop-price-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='PriceCalc.Ceiling/Ceiling10 with FeatureMatrix.GetFloat(ShopPriceRatio) en-US, 0 replaced by 1';x86=$x.cases;x64=$y.cases}|ConvertTo-Json -Depth 6 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-shop-price.json'),$result,[Text.UTF8Encoding]::new($false))
$diff=0;for($k=0;$k-lt$x.cases.Count;$k++){if(($x.cases[$k]|ConvertTo-Json -Compress)-cne($y.cases[$k]|ConvertTo-Json -Compress)){$diff++}}
Write-Output ('Shop price:'+$x.cases.Count+' cases per architecture;x86/x64 differences:'+$diff)
