[CmdletBinding()]
param(
 [Parameter(Mandatory)][ValidateSet('x86','x64')][string]$Architecture,
 [Parameter(Mandatory)][ValidateSet('invariant','en-US','zh-TW','zh-CN')][string]$Culture,
 [Parameter(Mandatory)][int]$ServiceId,
 [Parameter(Mandatory)][byte]$ChannelCode,
 [Parameter(Mandatory)][switch]$AllowlistStdin
)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$private=Join-Path $project '.cache/private-frontend'
$policyFile=Join-Path $project '.cache/private-content/reward-policy.json'
$source=Join-Path $project '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll.config'
$hash=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
if($hash-cne'62d60999723fff380a2575ee2f7b665a613d70028321e91991b21961a75a140b'){throw 'Candidate original configuration hash mismatch'}
if((Get-Item -LiteralPath $policyFile).Length-gt524288){throw 'Private policy exceeds bound'}
$policy=Get-Content -LiteralPath $policyFile -Raw|ConvertFrom-Json -DateKind String
if($policy.manifest.config_sha256-cne$hash){throw 'Original content manifest mismatch'}
[xml]$xml=[IO.File]::ReadAllText($source)
$ping=@();foreach($node in $xml.SelectNodes('//*')){foreach($attribute in $node.Attributes){if($attribute.Name-ieq'PingPong'){$ping+=$attribute.Value}}}
if($ping.Count-ne1-or$ping[0]-cne'false'){throw 'Candidate original PingPong changed'}
# Read at most16KiB of private operator allowlist from stdin. Never use original credentials.
$buffer=[char[]]::new(16385)
$count=[Console]::In.ReadBlock($buffer,0,$buffer.Length)
if($count-gt16384){[Array]::Clear($buffer);throw 'Private allowlist exceeds bound'}
$inputText=[string]::new($buffer,0,$count)
[Array]::Clear($buffer)
if([Text.Encoding]::UTF8.GetByteCount($inputText)-gt16384){throw 'Private allowlist exceeds bound'}
$list=ConvertFrom-Json -InputObject $inputText -NoEnumerate
if($list-isnot[Array]-or$list.Count-lt1-or$list.Count-gt8-or@($list|Where-Object{$_-isnot[string]-or$_.Length-gt1024}).Count-ne0){throw 'Invalid private allowlist'}
if((Get-Item -LiteralPath (Join-Path $project '.cache')).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Cache must not be a reparse point'}
if(Test-Path -LiteralPath $private){if((Get-Item -LiteralPath $private).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Private directory must not be a reparse point'}}
New-Item -ItemType Directory -Force -Path $private|Out-Null
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
$system=[Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
foreach($principal in @($sid,$system)){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($principal,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))}
[IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($private),$acl)
$actual=[IO.FileSystemAclExtensions]::GetAccessControl([IO.DirectoryInfo]::new($private))
$rules=@($actual.GetAccessRules($true,$false,[Security.Principal.SecurityIdentifier]))
if(-not$actual.AreAccessRulesProtected-or$rules.Count-ne2-or@($rules|Where-Object{$_.IdentityReference-ne$sid-and$_.IdentityReference-ne$system}).Count-ne0){throw 'Private ACL verification failed'}
$config=[ordered]@{format_version=1;bind='127.0.0.1:27015';framework_architecture=$Architecture;ascii_culture=$Culture;content_database_sha256=$policy.manifest.database_sha256;original_config_sha256=$hash;region=$policy.manifest.region;override_region=$policy.manifest.override_region;game_code=$policy.manifest.game_code;server_code=$policy.manifest.server_code;current_version=($policy.features|Where-Object { $_.key -ceq 'CurrentVer' }).value;service_id=$ServiceId;channel_code=$ChannelCode;civil_offset_seconds=28800;ping_pong=$false;allowed_passports=$list}
$output=Join-Path $private ('runtime-'+[Guid]::NewGuid().ToString('N')+'.json')
$stream=[IO.File]::Open($output,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try {$bytes=[Text.Encoding]::UTF8.GetBytes(($config|ConvertTo-Json -Depth 4)+"`n");$stream.Write($bytes);[Array]::Clear($bytes)}finally{$stream.Dispose()}
$fileAcl=[IO.FileSystemAclExtensions]::GetAccessControl([IO.FileInfo]::new($output))
$all=@($fileAcl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
if(@($all|Where-Object{$_.IdentityReference-ne$sid-and$_.IdentityReference-ne$system}).Count-ne0){throw 'Private file inherited unexpected principals'}
if((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()-cne$hash){throw 'Original configuration changed during read'}
[ordered]@{profile_path=$output;allowlisted_count=$list.Count;acl_verified=$true;original_configuration_unchanged=$true;source_ping_pong=$false;civil_offset_seconds=28800;listener_opened=$false;database_opened=$false;game_authentication_available=$false}|ConvertTo-Json -Compress
$inputText=$null;$list=$null;$config=$null
