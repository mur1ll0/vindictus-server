#requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ProfileFile)
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
$process=Start-Process -FilePath $exe -ArgumentList @('lab-frontend','--input-stdin','--policy-file',('"'+$policy+'"')) -WorkingDirectory $project -WindowStyle Hidden -Environment @{DATABASE_URL=$url;DATABASE_URL_FILE=$null;DB_MAX_CONNECTIONS='8';RUST_LOG='info'} -RedirectStandardInput $stdinCopy -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
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
[ordered]@{process_id=$process.Id;executable=$exe;binary_sha256=$binaryHash;stdout_log=$out;stderr_log=$err;profile_path=$profile;stdin_copy_path=$stdinCopy;started_at_utc=[DateTimeOffset]::UtcNow.ToString('o');loopback=$true;client_acceptance_verified=$false;character_creation_available=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $private 'runtime-process.json') -Encoding utf8NoBOM
[ordered]@{process_id=$process.Id;loopback=$true;profile_redirected_from_private_file=$true;logs_retained_locally=$true;client_acceptance_verified=$false;character_creation_available=$false}|ConvertTo-Json -Compress
