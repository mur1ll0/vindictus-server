SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
DECLARE @expected TABLE(c sysname,t int,l int,s int,n bit);
INSERT INTO @expected VALUES
('MailID',127,8,0,0),
('FromCID',127,8,0,0),
('FromName',231,512,0,0),
('ToCID',127,8,0,0),
('ToName',231,40,0,0),
('MailType',48,1,0,0),
('PostedTime',61,8,3,0),
('Title',231,512,0,0),
('Content',231,2048,0,0),
('ArriveTime',61,8,3,0),
('ExpireTime',61,8,3,1),
('IsRead',104,1,0,1),
('ChargedGold',56,4,0,0),
('IsDeleted',104,1,0,0);
IF EXISTS(SELECT * FROM @expected EXCEPT SELECT name,system_type_id,max_length,scale,is_nullable FROM sys.columns WHERE object_id=OBJECT_ID('dbo.Mail'))
 OR EXISTS(SELECT name,system_type_id,max_length,scale,is_nullable FROM sys.columns WHERE object_id=OBJECT_ID('dbo.Mail') EXCEPT SELECT * FROM @expected)
 THROW 50000,'Mail column metadata changed',1;
IF EXISTS(SELECT 1 FROM sys.columns WHERE object_id=OBJECT_ID('dbo.Mail') AND (is_identity=1 OR is_computed=1))
 OR EXISTS(SELECT 1 FROM sys.foreign_keys WHERE parent_object_id=OBJECT_ID('dbo.Mail'))
 OR (SELECT COUNT(*) FROM sys.indexes WHERE object_id=OBJECT_ID('dbo.Mail') AND is_unique=1)<>1
 OR NOT EXISTS(SELECT 1 FROM sys.indexes i JOIN sys.index_columns x ON x.object_id=i.object_id AND x.index_id=i.index_id JOIN sys.columns c ON c.object_id=x.object_id AND c.column_id=x.column_id WHERE i.object_id=OBJECT_ID('dbo.Mail') AND i.is_primary_key=1 AND i.is_disabled=0 AND x.key_ordinal=1 AND c.name='MailID')
 OR EXISTS(SELECT 1 FROM sys.index_columns x JOIN sys.indexes i ON i.object_id=x.object_id AND i.index_id=x.index_id WHERE i.object_id=OBJECT_ID('dbo.Mail') AND i.is_unique=1 AND x.key_ordinal>1)
 THROW 50000,'Mail key/identity/FK metadata changed',1;
IF (SELECT COUNT(*) FROM sys.default_constraints WHERE parent_object_id=OBJECT_ID('dbo.Mail'))<>4
 OR NOT EXISTS(SELECT 1 FROM sys.default_constraints WHERE parent_object_id=OBJECT_ID('dbo.Mail') AND COL_NAME(parent_object_id,parent_column_id)='FromCID' AND definition='((-1))')
 OR NOT EXISTS(SELECT 1 FROM sys.default_constraints WHERE parent_object_id=OBJECT_ID('dbo.Mail') AND COL_NAME(parent_object_id,parent_column_id)='ToCID' AND definition='((-1))')
 OR NOT EXISTS(SELECT 1 FROM sys.default_constraints WHERE parent_object_id=OBJECT_ID('dbo.Mail') AND COL_NAME(parent_object_id,parent_column_id)='IsRead' AND definition='((0))')
 OR NOT EXISTS(SELECT 1 FROM sys.default_constraints WHERE parent_object_id=OBJECT_ID('dbo.Mail') AND COL_NAME(parent_object_id,parent_column_id)='IsDeleted' AND definition='((0))')
 OR EXISTS(SELECT 1 FROM sys.columns WHERE object_id=OBJECT_ID('dbo.Mail') AND system_type_id=231 AND collation_name<>'SQL_Latin1_General_CP1_CI_AS')
 THROW 50000,'Mail defaults/collation changed',1;
IF (SELECT COUNT_BIG(*) FROM dbo.Mail)<>169 THROW 50000,'Mail source row profile changed',1;
SELECT OBJECT_DEFINITION(OBJECT_ID('dbo.QueryMailList')) AS procedure_definition,
JSON_QUERY((SELECT 1 AS format_version,'study_heroes.mail' AS source,
'c17336ad0fb0aca9d1cb6320580dd58538227ee48bd7a4292cf6e06cd33f484c' AS procedure_sha256,
JSON_QUERY(COALESCE((SELECT CONVERT(varchar(20),MailID) AS mail_id,CONVERT(varchar(20),FromCID) AS from_cid,
CONVERT(varchar(max),CONVERT(varbinary(max),FromName),2) AS from_name_hex,
CONVERT(varchar(20),ToCID) AS to_cid,CONVERT(varchar(max),CONVERT(varbinary(max),ToName),2) AS to_name_hex,
MailType AS mail_type,CONVERT(varchar(16),CONVERT(binary(8),PostedTime),2) AS posted_binary,
CONVERT(varchar(max),CONVERT(varbinary(max),Title),2) AS title_hex,
CONVERT(varchar(max),CONVERT(varbinary(max),Content),2) AS content_hex,
CONVERT(varchar(16),CONVERT(binary(8),ArriveTime),2) AS arrive_binary,
CONVERT(varchar(16),CONVERT(binary(8),ExpireTime),2) AS expire_binary,
IsRead AS is_read,ChargedGold AS charged_gold,IsDeleted AS is_deleted
FROM dbo.Mail ORDER BY MailID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS rows
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER)) AS snapshot
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
