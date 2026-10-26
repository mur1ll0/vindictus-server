#requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ProfileFile,[string]$NameProofFile,[string]$MinimumCreationAssetsFile,[ValidateRange(1,65535)][int]$BootstrapPort=8081,[switch]$SkipBootstrapHttp,[ValidateRange(0,65535)][int]$AdminPort=0)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$private=[IO.Path]::GetFullPath((Join-Path $project '.cache/private-frontend'))
$profile=[IO.Path]::GetFullPath($ProfileFile)
if(-not$profile.StartsWith($private+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)-or[IO.Path]::GetFileName($profile)-notmatch'^runtime-[0-9a-f]{32}\.json$'){throw 'Profile must be a prepared private laboratory file'}
foreach($path in @((Join-Path $project '.cache'),$private,$profile)){if((Get-Item -LiteralPath $path).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Private input must not be a reparse point'}}
if((Get-Item -LiteralPath $profile).Length-gt16384){throw 'Private profile exceeds bound'}
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
$system=[Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$acl=[IO.FileSystemAclExtensions]::GetAccessControl([IO.DirectoryInfo]::new($private))
if(-not$acl.AreAccessRulesProtected){throw 'Private directory ACL is not protected'}
$rules=@([IO.FileSystemAclExtensions]::GetAccessControl([IO.FileInfo]::new($profile)).GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
if($rules.Count-lt1-or@($rules|Where-Object{$_.IdentityReference-ne$sid-and$_.IdentityReference-ne$system}).Count-ne0){throw 'Private profile ACL contains unexpected principals'}
$settings=@{}
foreach($line in [IO.File]::ReadAllLines((Join-Path $project '.env'))){if($line-match'^(APP_DATABASE_USER|APP_DATABASE_PASSWORD|POSTGRES_DB|POSTGRES_HOST_PORT)=(.*)$'){$settings[$Matches[1]]=$Matches[2]}}
if(-not$settings.ContainsKey('APP_DATABASE_PASSWORD')-or[string]::IsNullOrEmpty($settings.APP_DATABASE_PASSWORD)){throw 'Local application role credential is required'}
$user=if($settings.ContainsKey('APP_DATABASE_USER')){$settings.APP_DATABASE_USER}else{'vindictus_app'}
$database=if($settings.ContainsKey('POSTGRES_DB')){$settings.POSTGRES_DB}else{'vindictus'}
$port=if($settings.ContainsKey('POSTGRES_HOST_PORT')){[int]$settings.POSTGRES_HOST_PORT}else{5433}
if($port-lt1-or$port-gt65535){throw 'Local PostgreSQL port is invalid'}
$url='postgres://'+[Uri]::EscapeDataString($user)+':'+[Uri]::EscapeDataString($settings.APP_DATABASE_PASSWORD)+'@127.0.0.1:'+ $port+'/'+[Uri]::EscapeDataString($database)
if(@(Get-NetTCPConnection -State Listen -LocalPort 27015 -ErrorAction SilentlyContinue).Count-gt0){throw 'Port 27015 is already in use; the Docker game service may be running (docker compose stop game)'}
$sourceExe=Join-Path $project 'target/debug/vindictus-server.exe'
$policy=Join-Path $project '.cache/private-content/reward-policy.json'
if(-not(Test-Path -LiteralPath $sourceExe)){throw 'Build the verified native binary first'}
$tag=[Guid]::NewGuid().ToString('N')
# Run a private copy so Windows does not lock the Cargo build output.
$exe=Join-Path $private ('runtime-'+$tag+'.exe')
[IO.File]::Copy($sourceExe,$exe,$false)
$binaryHash=(Get-FileHash -LiteralPath $sourceExe -Algorithm SHA256).Hash.ToLowerInvariant()
if((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()-cne$binaryHash){throw 'Runtime binary copy hash mismatch'}
$out=Join-Path $project ('.cache/verification/lab-frontend-'+$tag+'.stdout.log')
$err=Join-Path $project ('.cache/verification/lab-frontend-'+$tag+'.stderr.log')
# Keep the prepared profile readable by verification tools: Windows holds redirected stdin
# exclusively for the process lifetime. Use a separate bounded copy under the same protected ACL.
$stdinCopy=Join-Path $private ('stdin-'+$tag+'.json')
$inputBytes=[IO.File]::ReadAllBytes($profile)
try{
 $inputStream=[IO.File]::Open($stdinCopy,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$inputStream.Write($inputBytes)}finally{$inputStream.Dispose()}
}finally{[Array]::Clear($inputBytes);$inputBytes=$null}
$copyRules=@([IO.FileSystemAclExtensions]::GetAccessControl([IO.FileInfo]::new($stdinCopy)).GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
if($copyRules.Count-lt1-or@($copyRules|Where-Object{$_.IdentityReference-ne$sid-and$_.IdentityReference-ne$system}).Count-ne0){throw 'Private stdin copy ACL contains unexpected principals'}
# No secrets in arguments. The protected copy is redirected to bounded stdin.
$taskRuntimeArguments=@('lab-frontend','--input-stdin','--policy-file',('"'+$policy+'"'))
if($NameProofFile){
 $taskProof=[IO.Path]::GetFullPath($NameProofFile)
 $taskContentPrivate=[IO.Path]::GetFullPath((Join-Path $project '.cache/private-content'))
 if(-not$taskProof.StartsWith($taskContentPrivate+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)-or[IO.Path]::GetFileName($taskProof)-cne'character-name-proof.json'){throw 'Naming proof must be the prepared private laboratory file'}
 foreach($taskProofPath in @($taskContentPrivate,$taskProof)){if((Get-Item -LiteralPath $taskProofPath).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Private naming proof must not be a reparse point'}}
 if((Get-Item -LiteralPath $taskProof).Length-gt1MB){throw 'Naming proof exceeds bound'}
 $taskProofAcl=[IO.FileSystemAclExtensions]::GetAccessControl([IO.DirectoryInfo]::new($taskContentPrivate))
 $taskProofRules=@([IO.FileSystemAclExtensions]::GetAccessControl([IO.FileInfo]::new($taskProof)).GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
 if(-not$taskProofAcl.AreAccessRulesProtected-or$taskProofRules.Count-lt1-or@($taskProofRules|Where-Object{$_.IdentityReference-ne$sid-and$_.IdentityReference-ne$system}).Count-ne0){throw 'Private naming proof ACL is not protected'}
 $taskRuntimeArguments+=@('--name-proof-file',('"'+$taskProof+'"'))
}
if($MinimumCreationAssetsFile){
 if(-not$NameProofFile){throw 'Minimum creation requires the verified naming proof'}
 $taskAssets=[IO.Path]::GetFullPath($MinimumCreationAssetsFile)
 $taskAssetsRoot=[IO.Path]::GetFullPath((Join-Path $project '.cache/private-content'))
 if(-not$taskAssets.StartsWith($taskAssetsRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)-or[IO.Path]::GetFileName($taskAssets)-cne'minimum-creation-assets.json'){throw 'Minimum assets must use the prepared private configuration'}
 foreach($taskAssetsPath in @($taskAssetsRoot,$taskAssets)){if((Get-Item -LiteralPath $taskAssetsPath).Attributes-band[IO.FileAttributes]::ReparsePoint){throw 'Private assets configuration must not be a reparse point'}}
 if((Get-Item -LiteralPath $taskAssets).Length-gt16384){throw 'Private assets configuration exceeds bound'}
 $taskAssetsRules=@([IO.FileSystemAclExtensions]::GetAccessControl([IO.FileInfo]::new($taskAssets)).GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
 if($taskAssetsRules.Count-lt1-or@($taskAssetsRules|Where-Object{$_.IdentityReference-ne$sid-and$_.IdentityReference-ne$system}).Count-ne0){throw 'Private assets configuration ACL is not protected'}
 $taskRuntimeArguments+=@('--minimum-creation-assets',('"'+$taskAssets+'"'))
}
# Published content (B4.4c): ShopList.aspx for the bootstrap HTTP and the overlay for the client launcher.
$taskState=Join-Path $project '.cache/content-state';[void](New-Item -ItemType Directory -Force $taskState)
$taskRuntimeArguments+=@('--shop-list-output',('"'+(Join-Path $project '.cache/client-bootstrap/en-eu-x-gm/ShopList.aspx')+'"'),'--content-edits-output',('"'+(Join-Path $taskState 'content-edits.json')+'"'))
# Admin panel (B4): loopback listener; serves web/admin/dist when it was built.
if($AdminPort-gt0){
 $taskRuntimeArguments+=@('--admin-listen',('127.0.0.1:'+$AdminPort))
 $taskUi=Join-Path $project 'web/admin/dist'
 if(Test-Path -LiteralPath (Join-Path $taskUi 'index.html')){$taskRuntimeArguments+=@('--admin-ui-dir',('"'+$taskUi+'"'))}
 $taskNames=Join-Path $project '.cache/private-content/item-names.json'
 if(Test-Path -LiteralPath $taskNames){$taskRuntimeArguments+=@('--admin-item-names',('"'+$taskNames+'"'))
  $taskCatalog=Join-Path $project '.cache/private-content/panel-catalog.json'
  if(Test-Path -LiteralPath $taskCatalog){$taskRuntimeArguments+=@('--admin-panel-catalog',('"'+$taskCatalog+'"'))}}
 $taskIcons=Join-Path $project '.cache/private-content/icons'
 if(Test-Path -LiteralPath $taskIcons){$taskRuntimeArguments+=@('--admin-icons-dir',('"'+$taskIcons+'"'))}
}
$process=Start-Process -FilePath $exe -ArgumentList $taskRuntimeArguments -WorkingDirectory $project -WindowStyle Hidden -Environment @{DATABASE_URL=$url;DATABASE_URL_FILE=$null;DB_MAX_CONNECTIONS='8';RUST_LOG='info'} -RedirectStandardInput $stdinCopy -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
$settings.Clear();$url=$null
$deadline=[DateTime]::UtcNow.AddSeconds(20)
$ready=$false
while([DateTime]::UtcNow-lt$deadline-and-not$process.HasExited){
 if(Test-Path -LiteralPath $err){
  $stream=[IO.File]::Open($err,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
  try{
   $reader=[IO.StreamReader]::new($stream)
   for($i=0;$i-lt16-and-not$reader.EndOfStream;$i++){
    $line=$reader.ReadLine();if($line.Length-gt8192){throw 'Unexpected startup log bound'}
    try{$record=$line|ConvertFrom-Json -DateKind String}catch{continue}
    if($record.fields.message-ceq'restricted laboratory frontend listening'){$ready=$true;break}
   }
  }finally{$stream.Dispose()}
 }
 if($ready){break};Start-Sleep -Milliseconds 250
}
if(-not$ready){if(-not$process.HasExited){$process.Kill();$process.WaitForExit()};throw 'Laboratory frontend did not start; complete logs retained locally'}
# The client reads its server list (EndPoint.txt) over HTTP before connecting; without it the list is empty.
$bootstrapRoot=Join-Path $project '.cache/client-bootstrap'
$bootstrapPid=$null
if(-not$SkipBootstrapHttp){
 if(-not(Test-Path -LiteralPath (Join-Path $bootstrapRoot 'en-eu-x-gm/EndPoint.txt'))){throw 'Prepare the client bootstrap first (scripts/client/prepare_lab_client.py --apply)'}
 $listening=@(Get-NetTCPConnection -State Listen -LocalPort $BootstrapPort -ErrorAction SilentlyContinue)
 if($listening.Count-eq0){
  $bootstrap=Start-Process -FilePath 'py' -ArgumentList @('-3.13','-m','http.server',[string]$BootstrapPort,'--bind','127.0.0.1','--directory',('"'+$bootstrapRoot+'"')) -WorkingDirectory $project -WindowStyle Hidden -RedirectStandardOutput (Join-Path $project '.cache/verification/lab-bootstrap-http.stdout.log') -RedirectStandardError (Join-Path $project '.cache/verification/lab-bootstrap-http.stderr.log') -PassThru
  $bootstrapPid=$bootstrap.Id
 }else{$bootstrapPid=$listening[0].OwningProcess}
 $bootstrapDeadline=[DateTime]::UtcNow.AddSeconds(10);$bootstrapReady=$false
 while([DateTime]::UtcNow-lt$bootstrapDeadline-and-not$bootstrapReady){
  try{$bootstrapReady=(Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 -Uri ('http://127.0.0.1:'+$BootstrapPort+'/en-eu-x-gm/EndPoint.txt')).StatusCode-eq200}catch{Start-Sleep -Milliseconds 250}
 }
 if(-not$bootstrapReady){throw 'Client bootstrap HTTP did not answer; the client would show no servers'}
}
[ordered]@{process_id=$process.Id;executable=$exe;binary_sha256=$binaryHash;stdout_log=$out;stderr_log=$err;profile_path=$profile;stdin_copy_path=$stdinCopy;started_at_utc=[DateTimeOffset]::UtcNow.ToString('o');loopback=$true;client_acceptance_verified=$false;character_creation_available=[bool]$MinimumCreationAssetsFile}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $private 'runtime-process.json') -Encoding utf8NoBOM
[ordered]@{process_id=$process.Id;bootstrap_http_process_id=$bootstrapPid;loopback=$true;profile_redirected_from_private_file=$true;logs_retained_locally=$true;client_acceptance_verified=$false;character_creation_available=[bool]$MinimumCreationAssetsFile}|ConvertTo-Json -Compress
