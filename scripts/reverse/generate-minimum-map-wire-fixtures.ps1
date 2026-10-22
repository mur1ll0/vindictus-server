$ErrorActionPreference = 'Stop'
$taskProject = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
    New-Item -ItemType Directory -Force .cache/framework, .cache/verification | Out-Null

    $taskTypes = @(
        ,@('NoticeGameEnvironmentMessage', 345, '97636de8-6ff1-3d4c-8c53-b8f1da851df4')
        ,@('Identify', 363, '428dee64-190d-3cf6-8450-3e511769c608')
        ,@('EnterChannel', 364, '79ce9a53-8956-3645-a67b-f69681ea7a35')
        ,@('ChannelServerAddress', 365, '85bf1995-487c-3e7e-8603-e88d75d21b46')
        ,@('IdentifyFailed', 366, '42fa5d33-391e-320b-85db-0c61e1b7f6ba')
        ,@('MovePartition', 368, 'd3cf4e65-f3a6-37f4-aae6-d8819a02e728')
        ,@('UpdateAction', 369, 'bb98e908-edbe-32d5-89c2-eb38d6508cad')
        ,@('Disappeared', 372, '4f13a705-3f02-37cb-b869-ebfbf3852ce2')
        ,@('NotifyLook', 373, '711f48ae-c601-344c-bb5a-9320b001f809')
        ,@('NotifyAction', 374, '86b97474-ac74-3722-bda1-a8ba43968733')
        ,@('SelectCharacterMessage', 481, '1f34c672-5581-353f-9944-d5ab72e808a5')
        ,@('LoginOkMessage', 483, 'e301ee14-10c2-3886-8533-fbba9ef1cbe1')
        ,@('CharacterCommonInfoMessage', 609, '9756914f-6753-3228-b0b8-38b30dc1a612')
        ,@('UpdateStatMessage', 619, 'a1407a79-5353-3455-9a2c-859f8825c200')
        ,@('LevelUpMessage', 476, '23314733-9565-36b4-a382-8042ea30debe')
    )

    $taskSources = @('scripts/reverse/framework/MinimumMapWireOracle.cs')
    foreach ($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs') {
        $taskSources += ('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/' + $taskFile)
    }
    $taskSources += @(
        'legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs',
        'legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs',
        'legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs',
        'legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs',
        'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs',
        'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/ActionSync.cs',
        'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/Vector3D.cs'
    )
    foreach ($taskFile in 'CostumeInfo.cs','PetStatusInfo.cs','PetStatElement.cs','PetSkillElement.cs','PetAccessoryElement.cs') {
        $taskSources += ('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/' + $taskFile)
    }
    foreach ($taskFile in 'CharacterSummary.cs','BaseCharacter.cs','VocationEnum.cs') {
        $taskSources += ('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/' + $taskFile)
    }
    foreach ($taskFile in 'Extention.cs','DictionaryExtension.cs','MultiDictionary.cs') {
        $taskSources += ('legacy/decompiled/Utility-37359981d509/Utility/' + $taskFile)
    }
    foreach ($taskFile in 'Channel.cs','ComponentSpace.cs','Disappeared.cs','ICamera.cs','IChannel.cs','IComponent.cs','IComponentSpace.cs','IMessage.cs','IModifier.cs','IPartition.cs','IRegion.cs','Key.cs','KeyUtil.cs','Location.cs','MovePartition.cs','Partition.cs','Region.cs') {
        $taskSources += ('legacy/decompiled/MMOServer-801827edfc83/MMOServer/' + $taskFile)
    }
    foreach ($taskFile in 'Kinetics.cs','KineticsChanged.cs','Look.cs','Map.cs','MapFile.cs','UpdateLook.cs') {
        $taskSources += ('legacy/decompiled/HeroesChannelServer-3264f671bce0/HeroesChannelServer/' + $taskFile)
    }
    $taskSources += 'legacy/decompiled/HeroesChannelServer-3264f671bce0/HeroesChannelServer.Message/PacketMessage.cs'

    $taskOriginalHash = 'be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
    if ((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant() -ne $taskOriginalHash) {
        throw 'Original metadata assembly hash changed'
    }

    $taskOriginalPaths = @()
    $taskCopies = @()
    foreach ($taskType in $taskTypes) {
        foreach ($taskArch in 'x86','x64') {
            $taskTable = Get-Content ('docs/evidence/reverse/framework-types-' + $taskArch + '.json') -Raw | ConvertFrom-Json
            $taskRow = @($taskTable.messages | Where-Object name -eq ('ServiceCore.EndPointNetwork.' + $taskType[0]))
            if ($taskTable.source_sha256 -ne $taskOriginalHash -or $taskRow.Count -ne 1 -or $taskRow[0].category -ne $taskType[1] -or $taskRow[0].guid -ne $taskType[2]) {
                throw ('Original type identity mismatch: ' + $taskType[0])
            }
        }
        $taskSource = 'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/' + $taskType[0] + '.cs'
        $taskCode = [IO.File]::ReadAllText((Join-Path $PWD $taskSource))
        if (([regex]::Matches($taskCode, '\[Serializable\]')).Count -ne 1) {
            throw ('Unexpected source type shape: ' + $taskType[0])
        }
        $taskCode = $taskCode.Replace('[Serializable]', '[Serializable, System.Runtime.InteropServices.Guid("' + $taskType[2] + '")]')
        $taskCopy = '.cache/framework/MinimumMap-' + $taskType[0] + '.cs'
        [IO.File]::WriteAllText((Join-Path $PWD $taskCopy), $taskCode, [Text.UTF8Encoding]::new($false))
        $taskOriginalPaths += $taskSource
        $taskCopies += $taskCopy
    }

    $taskMapDir = '../ServidorVindictus/Server/zh-tw-x-gm/Data/MapPartition'
    $taskMapHashes = @(Get-ChildItem $taskMapDir -Filter '*.dat' | Sort-Object Name | ForEach-Object {
        [ordered]@{
            name = $_.Name
            length = $_.Length
            sha256 = (Get-FileHash $_.FullName).Hash.ToLowerInvariant()
        }
    })
    if ($taskMapHashes.Count -ne 8) {
        throw 'Unexpected MapPartition .dat count'
    }

    $taskProfiles = @()
    foreach ($taskArch in 'x86','x64') {
        $taskExe = '.cache/framework/MinimumMapWire-' + $taskArch + '.exe'
        $taskArgs = @(
            'C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll',
            '/nologo',
            '/noconfig',
            '/nostdlib+',
            '/unsafe+',
            '/langversion:latest',
            ('/platform:' + $taskArch),
            ('/out:' + $taskExe)
        )
        foreach ($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions') {
            $taskArgs += ('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/' + $taskLibrary + '.dll')
        }
        $taskArgs += $taskSources
        $taskArgs += $taskCopies
        & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/minimum-map-wire-build-' + $taskArch + '.log')
        if ($LASTEXITCODE -ne 0) {
            throw ('Minimum map wire oracle compilation failed (' + $taskArch + '); see .cache/verification')
        }
        if (Select-String ('.cache/verification/minimum-map-wire-build-' + $taskArch + '.log') -Pattern 'warning [A-Z]+[0-9]+:') {
            throw ('Minimum map wire oracle compiler warning (' + $taskArch + ')')
        }
        & (Join-Path $PWD $taskExe) $taskMapDir *> ('.cache/verification/minimum-map-wire-' + $taskArch + '.json')
        if ($LASTEXITCODE -ne 0) {
            throw ('Minimum map wire oracle execution failed (' + $taskArch + '); see .cache/verification')
        }
        $taskProfile = Get-Content ('.cache/verification/minimum-map-wire-' + $taskArch + '.json') -Raw | ConvertFrom-Json
        if ($taskProfile.architecture -ne $taskArch) {
            throw 'Oracle architecture mismatch'
        }
        $taskProfiles += $taskProfile
        Write-Output ('Minimum map oracle: ' + $taskArch + '; wire_cases=' + $taskProfile.wire_cases.Count + '; maps=' + $taskProfile.maps.Count + '; neighbors=' + $taskProfile.neighbors.Count + '; steps=' + $taskProfile.channel_steps.Count)
    }

    foreach ($taskField in 'wire_cases','epoch_cases','maps','neighbors','channel_steps') {
        if (($taskProfiles[0].$taskField | ConvertTo-Json -Depth 24 -Compress) -cne ($taskProfiles[1].$taskField | ConvertTo-Json -Depth 24 -Compress)) {
            throw ('Minimum map oracle architecture disagreement in ' + $taskField)
        }
    }

    $taskAllSources = $taskSources + $taskOriginalPaths + @('scripts/reverse/generate-minimum-map-wire-fixtures.ps1')
    $taskResult = [ordered]@{
        profiles = $taskProfiles
        source_sha256 = $taskOriginalHash
        map_files = $taskMapHashes
        sources = @($taskAllSources | ForEach-Object {
            [ordered]@{
                path = $_
                sha256 = (Get-FileHash $_).Hash.ToLowerInvariant()
            }
        })
        original_services_executed = $false
        method = 'Recovered Framework serializer/reader, MessageAnalyzer(null crypto), Utility.Extention.ToInt32, HeroesChannelServer MapFile/Map/Look/Kinetics, and MMOServer Channel/Region/Partition/ComponentSpace.'
    }
    $taskResult | ConvertTo-Json -Depth 24 -Compress | Set-Content 'tests/fixtures/framework-minimum-map-wire.json' -Encoding utf8NoBOM
    Write-Output 'Saved tests/fixtures/framework-minimum-map-wire.json with 0 x86/x64 disagreements.'
} finally {
    Pop-Location
}
