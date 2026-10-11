-- SQL Server 2022; called by Restore-Legacy.ps1 with validated sqlcmd variables.
SET NOCOUNT ON;
SET XACT_ABORT ON;
DECLARE @database sysname = N'$(DatabaseName)';
DECLARE @backup nvarchar(4000) = N'/backups/$(BackupName)';
IF DB_ID(@database) IS NOT NULL THROW 50001, 'Refusing to restore over an existing database.', 1;

CREATE TABLE #files (
    LogicalName nvarchar(128), PhysicalName nvarchar(260), Type char(1), FileGroupName nvarchar(128),
    Size numeric(20,0), MaxSize numeric(20,0), FileID bigint, CreateLSN numeric(25,0),
    DropLSN numeric(25,0), UniqueID uniqueidentifier, ReadOnlyLSN numeric(25,0),
    ReadWriteLSN numeric(25,0), BackupSizeInBytes bigint, SourceBlockSize int, FileGroupID int,
    LogGroupGUID uniqueidentifier, DifferentialBaseLSN numeric(25,0), DifferentialBaseGUID uniqueidentifier,
    IsReadOnly bit, IsPresent bit, TDEThumbprint varbinary(32), SnapshotURL nvarchar(360)
);
DECLARE @inspect nvarchar(max) = N'RESTORE FILELISTONLY FROM DISK = N''' + REPLACE(@backup, '''', '''''') + N''' WITH FILE = 1';
INSERT INTO #files EXEC(@inspect);
IF NOT EXISTS (SELECT 1 FROM #files) THROW 50002, 'Backup contains no logical files.', 1;
IF EXISTS (SELECT 1 FROM #files WHERE IsPresent = 0) THROW 50003, 'Partial backup requires a separate restore plan.', 1;

-- MOVE every file; never assume a backup has only one MDF and one LDF.
DECLARE @move nvarchar(max);
SELECT @move = STRING_AGG(CAST(
    N'MOVE N''' + REPLACE(LogicalName, '''', '''''') + N''' TO N''/var/opt/mssql/data/'
    + @database + N'_' + CONVERT(nvarchar(20), FileID)
    + CASE Type WHEN 'L' THEN N'.ldf' WHEN 'D' THEN N'.mdf' ELSE N'_container' END + N''''
    AS nvarchar(max)), N', ') WITHIN GROUP (ORDER BY FileID)
FROM #files;
DECLARE @restore nvarchar(max) = N'RESTORE DATABASE ' + QUOTENAME(@database)
    + N' FROM DISK = N''' + REPLACE(@backup, '''', '''''') + N''' WITH FILE = 1, '
    + @move + N', RECOVERY, STATS = 10';
PRINT @restore;
EXEC(@restore);
DBCC CHECKDB (@database) WITH NO_INFOMSGS;
