[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $taskSource=(Resolve-Path -LiteralPath '..\ServidorVindictus\Server\zh-tw-x-gm\Bin\ServiceCore.dll').Path
    $taskFrameworkRoot=Join-Path $env:WINDIR 'Microsoft.NET'
    New-Item -ItemType Directory -Force .cache\framework | Out-Null
    foreach($taskPlatform in 'x86','x64') {
        $taskCompilerFolder=if($taskPlatform -eq 'x64'){'Framework64'}else{'Framework'}
        $taskFramework=Join-Path $taskFrameworkRoot ($taskCompilerFolder+'\v4.0.30319\csc.exe')
        $taskExe=Join-Path $taskProject ('.cache\framework\FrameworkTypes-'+$taskPlatform+'.exe')
        & $taskFramework /nologo ('/platform:'+$taskPlatform) /r:System.Web.Extensions.dll ('/out:'+$taskExe) scripts\reverse\framework\FrameworkTypes.cs
        if($LASTEXITCODE -ne 0){throw 'Framework metadata oracle compile failed'}
        $taskOutput=& $taskExe $taskSource
        if($LASTEXITCODE -ne 0){throw ('ReflectionOnly metadata failed for '+$taskPlatform)}
        $taskData=$taskOutput -join "`n" | ConvertFrom-Json
        $taskFile=Join-Path $taskProject ('docs\evidence\reverse\framework-types-'+$taskPlatform+'.json')
        [IO.File]::WriteAllText($taskFile,($taskData | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
        Write-Host ($taskPlatform+': ReflectionOnly types '+$taskData.messages.Count+'; runtime '+$taskData.runtime)
    }
} finally {Pop-Location}
