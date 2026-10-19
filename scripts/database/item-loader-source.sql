SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
DECLARE @expected TABLE(t sysname,c sysname,typ int,len int,scale int,nullable bit,ident bit);
INSERT INTO @expected VALUES
('ExpandSlotInfo','ID',56,4,0,0,1),
('ExpandSlotInfo','CID',127,8,0,0,0),
('ExpandSlotInfo','Slot',56,4,0,0,0),
('NewRecipe','RowID',127,8,0,0,1),
('NewRecipe','CID',127,8,0,0,0),
('NewRecipe','RecipeID',231,200,0,0,0),
('NewRecipe','NewUntil',42,8,7,0,0),
('ManufactureRecipe','rowID',56,4,0,0,1),
('ManufactureRecipe','CID',127,8,0,0,0),
('ManufactureRecipe','RecipeID',231,256,0,0,0),
('ManufactureRecipe','AcquireTime',42,8,7,0,0),
('Manufacture','rowID',56,4,0,0,1),
('Manufacture','CID',127,8,0,0,0),
('Manufacture','ManufacturelID',231,32,0,0,0),
('Manufacture','Grade',56,4,0,0,0),
('Manufacture','ExperiencePoint',56,4,0,0,0),
('QuickSlot','ItemClass',231,100,0,0,0),
('QuickSlot','ROWID',127,8,0,0,1),
('QuickSlot','OwnerID',127,8,0,0,0),
('QuickSlot','Slot',56,4,0,0,0);
IF EXISTS(SELECT * FROM @expected EXCEPT SELECT OBJECT_NAME(c.object_id),c.name,c.system_type_id,c.max_length,c.scale,c.is_nullable,c.is_identity FROM sys.columns c WHERE c.object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe')))
 OR EXISTS(SELECT OBJECT_NAME(c.object_id),c.name,c.system_type_id,c.max_length,c.scale,c.is_nullable,c.is_identity FROM sys.columns c WHERE c.object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe') ) EXCEPT SELECT * FROM @expected)
 THROW 50000,'Item loader column metadata changed',1;
IF (SELECT COUNT(*) FROM sys.identity_columns WHERE object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe') ) AND CONVERT(bigint,seed_value)=1 AND CONVERT(bigint,increment_value)=1)<>5
 THROW 50000,'Item loader identity metadata changed',1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe') ) AND is_unique=1 AND is_primary_key=1)<>5
 OR (SELECT COUNT(*) FROM sys.indexes WHERE object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe') ) AND is_unique=1)<>5
 THROW 50000,'Item loader unique keys changed',1;
IF (SELECT COUNT(*) FROM sys.foreign_keys WHERE parent_object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe') ))<>2
 OR (SELECT COUNT(*) FROM sys.foreign_keys WHERE ((parent_object_id=OBJECT_ID('dbo.QuickSlot') AND referenced_object_id=OBJECT_ID('dbo.Character')) OR (parent_object_id=OBJECT_ID('dbo.NewRecipe') AND referenced_object_id=OBJECT_ID('dbo.ItemCollection'))) AND delete_referential_action=1 AND update_referential_action=1 AND is_disabled=0 AND is_not_trusted=0)<>2
 THROW 50000,'Item loader foreign keys changed',1;
IF EXISTS(SELECT 1 FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id=i.object_id AND ic.index_id=i.index_id JOIN sys.columns c ON c.object_id=ic.object_id AND c.column_id=ic.column_id WHERE i.is_primary_key=1 AND i.object_id IN (OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe') ) AND (c.name NOT IN ('ROWID','RowID','rowID','ID') OR ic.key_ordinal<>1))
 THROW 50000,'Item loader primary key columns changed',1;
IF (SELECT COUNT(*) FROM sys.foreign_key_columns f JOIN sys.columns pc ON pc.object_id=f.parent_object_id AND pc.column_id=f.parent_column_id JOIN sys.columns rc ON rc.object_id=f.referenced_object_id AND rc.column_id=f.referenced_column_id WHERE ((f.parent_object_id=OBJECT_ID('dbo.QuickSlot') AND f.referenced_object_id=OBJECT_ID('dbo.Character') AND pc.name='OwnerID' AND rc.name='ID') OR (f.parent_object_id=OBJECT_ID('dbo.NewRecipe') AND f.referenced_object_id=OBJECT_ID('dbo.ItemCollection') AND pc.name='CID' AND rc.name='OwnerID')))<>2
 THROW 50000,'Item loader foreign key columns changed',1;
IF EXISTS(SELECT 1 FROM sys.default_constraints WHERE parent_object_id IN(OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.ExpandSlotInfo'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe')))
 THROW 50000,'Item loader defaults changed',1;
IF (SELECT COUNT_BIG(*) FROM dbo.QuickSlot)<>261 OR EXISTS(SELECT 1 FROM dbo.ExpandSlotInfo) OR EXISTS(SELECT 1 FROM dbo.Manufacture) OR EXISTS(SELECT 1 FROM dbo.ManufactureRecipe) OR EXISTS(SELECT 1 FROM dbo.NewRecipe)
 THROW 50000,'Item loader source row profile changed',1;
IF EXISTS(SELECT 1 FROM sys.columns WHERE object_id IN(OBJECT_ID('dbo.QuickSlot'),OBJECT_ID('dbo.Manufacture'),OBJECT_ID('dbo.ManufactureRecipe'),OBJECT_ID('dbo.NewRecipe')) AND system_type_id=231 AND collation_name<>'SQL_Latin1_General_CP1_CI_AS')
 THROW 50000,'Item loader source collation changed',1;
SELECT 1 AS format_version,'study_heroes.item-loader-state' AS source,'__SCHEMA_SHA256__' AS schema_sha256,
JSON_QUERY(CONCAT('[',(SELECT COALESCE('"'+CONVERT(varchar(20),last_value)+'"','null') FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.QuickSlot')),',',(SELECT COALESCE('"'+CONVERT(varchar(20),last_value)+'"','null') FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.ExpandSlotInfo')),',',(SELECT COALESCE('"'+CONVERT(varchar(20),last_value)+'"','null') FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.Manufacture')),',',(SELECT COALESCE('"'+CONVERT(varchar(20),last_value)+'"','null') FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.ManufactureRecipe')),',',(SELECT COALESCE('"'+CONVERT(varchar(20),last_value)+'"','null') FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.NewRecipe')),']')) AS source_last,
JSON_QUERY(COALESCE((SELECT CONVERT(varchar(20),ROWID) AS row_id,CONVERT(varchar(20),OwnerID) AS owner_id,CONVERT(varchar(max),CONVERT(varbinary(max),ItemClass),2) AS item_class_utf16,Slot AS slot FROM dbo.QuickSlot ORDER BY ROWID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS quick_slots,
JSON_QUERY(COALESCE((SELECT ID AS row_id,CONVERT(varchar(20),CID) AS cid,Slot AS slot FROM dbo.ExpandSlotInfo ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS expanded_slots,
JSON_QUERY(COALESCE((SELECT rowID AS row_id,CONVERT(varchar(20),CID) AS cid,CONVERT(varchar(max),CONVERT(varbinary(max),ManufacturelID),2) AS manufacture_utf16,Grade AS grade,ExperiencePoint AS experience_point FROM dbo.Manufacture ORDER BY rowID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS manufacture,
JSON_QUERY(COALESCE((SELECT rowID AS row_id,CONVERT(varchar(20),CID) AS cid,CONVERT(varchar(max),CONVERT(varbinary(max),RecipeID),2) AS recipe_utf16,CONVERT(varchar(20),DATEDIFF_BIG(SECOND,CONVERT(datetime2(7),'0001-01-01'),AcquireTime)*10000000+DATEPART(NANOSECOND,AcquireTime)/100) AS acquired_ticks FROM dbo.ManufactureRecipe ORDER BY rowID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS recipes,
JSON_QUERY(COALESCE((SELECT CONVERT(varchar(20),RowID) AS row_id,CONVERT(varchar(20),CID) AS cid,CONVERT(varchar(max),CONVERT(varbinary(max),RecipeID),2) AS recipe_utf16,CONVERT(varchar(20),DATEDIFF_BIG(SECOND,CONVERT(datetime2(7),'0001-01-01'),NewUntil)*10000000+DATEPART(NANOSECOND,NewUntil)/100) AS new_until_ticks FROM dbo.NewRecipe ORDER BY RowID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS new_recipes
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
