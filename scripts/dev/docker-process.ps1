function Invoke-ProjectDocker {
    param([Parameter(Mandatory)][string[]]$Arguments, [AllowNull()][string]$InputText = $null)
    $taskStart = [Diagnostics.ProcessStartInfo]::new('docker')
    $taskStart.WorkingDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
    $taskStart.UseShellExecute = $false
    $taskStart.CreateNoWindow = $true
    $taskStart.RedirectStandardOutput = $true
    $taskStart.RedirectStandardError = $true
    $taskStart.RedirectStandardInput = $true
    $taskStart.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
    $taskStart.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $taskStart.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    foreach ($taskArgument in $Arguments) { $taskStart.ArgumentList.Add($taskArgument) }
    $taskProcess = [Diagnostics.Process]::new()
    $taskProcess.StartInfo = $taskStart
    try {
        if (-not $taskProcess.Start()) { throw 'Could not start Docker.' }
        $taskStdout = $taskProcess.StandardOutput.ReadToEndAsync()
        $taskStderr = $taskProcess.StandardError.ReadToEndAsync()
        if ($null -ne $InputText) { $taskProcess.StandardInput.Write($InputText) }
        $taskProcess.StandardInput.Close()
        $taskProcess.WaitForExit()
        [pscustomobject]@{ ExitCode = $taskProcess.ExitCode; Stdout = $taskStdout.GetAwaiter().GetResult(); Stderr = $taskStderr.GetAwaiter().GetResult() }
    } finally { $taskProcess.Dispose() }
}
