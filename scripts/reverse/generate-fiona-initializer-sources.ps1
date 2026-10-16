$ErrorActionPreference='Stop'
$root=$PWD.Path
# Code provenance only: no private profile, database, feature key/value or export is read.
$files=[ordered]@{
 contents='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterContents.cs'
 skills='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillManager.cs'
 effects='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
 stats='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/StatManager.cs'
 ap='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/APManager.cs'
 titles='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/TitleManager.cs'
 loader='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterLoader.cs'
 query='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterDataContext.cs'
 feature_matrix='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
}
$result=[ordered]@{}
foreach($name in $files.Keys){$path=[IO.Path]::GetFullPath((Join-Path $root $files[$name]));if(-not$path.StartsWith((Join-Path $root 'legacy/decompiled')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)-or[IO.Path]::GetExtension($path)-cne'.cs'){throw 'Source provenance path outside decompiled code'};$result[$name]=[ordered]@{path=$files[$name];sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}}
$json=$result|ConvertTo-Json -Depth 4
[IO.File]::WriteAllText((Join-Path $root 'src/domain/fiona_initializer_sources.json'),$json.Replace("`r`n","`n")+"`n",[Text.UTF8Encoding]::new($false))
Write-Output ('Initializer code provenance:files='+$result.Count+';only paths and SHA256;no private data read or published.')
