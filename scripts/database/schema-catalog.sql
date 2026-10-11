-- Read-only schema evidence, including complete persisted SQL module definitions.
-- JSON catalog is for analysis; not a PostgreSQL import or SQL Server DDL replay script.
SET NOCOUNT ON;
SELECT
    DB_NAME() AS database_name,
    CAST(DATABASEPROPERTYEX(DB_NAME(), 'Collation') AS nvarchar(128)) AS collation,
    (SELECT compatibility_level FROM sys.databases WHERE name = DB_NAME()) AS compatibility_level,
    JSON_QUERY((SELECT * FROM sys.schemas FOR JSON PATH)) AS schemas,
    JSON_QUERY((SELECT * FROM sys.objects WHERE is_ms_shipped = 0 FOR JSON PATH)) AS objects,
    JSON_QUERY((SELECT * FROM sys.tables WHERE is_ms_shipped = 0 FOR JSON PATH)) AS tables,
    JSON_QUERY((SELECT c.*, TYPE_NAME(c.user_type_id) AS declared_type FROM sys.columns c JOIN sys.objects o ON o.object_id = c.object_id WHERE o.is_ms_shipped = 0 FOR JSON PATH)) AS columns,
    JSON_QUERY((SELECT * FROM sys.types WHERE is_user_defined = 1 FOR JSON PATH)) AS user_types,
    JSON_QUERY((SELECT * FROM sys.table_types FOR JSON PATH)) AS table_types,
    JSON_QUERY((SELECT * FROM sys.default_constraints WHERE is_ms_shipped = 0 FOR JSON PATH)) AS default_constraints,
    JSON_QUERY((SELECT * FROM sys.check_constraints WHERE is_ms_shipped = 0 FOR JSON PATH)) AS check_constraints,
    JSON_QUERY((SELECT * FROM sys.computed_columns FOR JSON PATH)) AS computed_columns,
    JSON_QUERY((SELECT * FROM sys.identity_columns FOR JSON PATH)) AS identity_columns,
    JSON_QUERY((SELECT * FROM sys.key_constraints WHERE is_ms_shipped = 0 FOR JSON PATH)) AS key_constraints,
    JSON_QUERY((SELECT * FROM sys.foreign_keys WHERE is_ms_shipped = 0 FOR JSON PATH)) AS foreign_keys,
    JSON_QUERY((SELECT * FROM sys.foreign_key_columns FOR JSON PATH)) AS foreign_key_columns,
    JSON_QUERY((SELECT i.* FROM sys.indexes i JOIN sys.objects o ON o.object_id = i.object_id WHERE o.is_ms_shipped = 0 FOR JSON PATH)) AS indexes,
    JSON_QUERY((SELECT c.* FROM sys.index_columns c JOIN sys.objects o ON o.object_id = c.object_id WHERE o.is_ms_shipped = 0 FOR JSON PATH)) AS index_columns,
    JSON_QUERY((SELECT * FROM sys.sql_modules FOR JSON PATH)) AS sql_modules,
    JSON_QUERY((SELECT p.object_id, p.name, p.parameter_id, p.system_type_id, p.user_type_id, p.max_length, p.precision, p.scale, p.is_output, p.is_cursor_ref, p.has_default_value, CONVERT(nvarchar(max), p.default_value) AS default_value, p.is_xml_document, p.xml_collection_id, p.is_readonly FROM sys.parameters p JOIN sys.objects o ON o.object_id = p.object_id WHERE o.is_ms_shipped = 0 FOR JSON PATH)) AS parameters,
    JSON_QUERY((SELECT * FROM sys.assemblies WHERE is_user_defined = 1 FOR JSON PATH)) AS assemblies,
    JSON_QUERY((SELECT * FROM sys.assembly_modules FOR JSON PATH)) AS assembly_modules,
    JSON_QUERY((SELECT * FROM sys.partition_functions FOR JSON PATH)) AS partition_functions,
    JSON_QUERY((SELECT * FROM sys.partition_schemes FOR JSON PATH)) AS partition_schemes,
    JSON_QUERY((SELECT * FROM sys.triggers WHERE is_ms_shipped = 0 FOR JSON PATH)) AS triggers,
    JSON_QUERY((SELECT * FROM sys.synonyms FOR JSON PATH)) AS synonyms,
    JSON_QUERY((SELECT object_id, name, schema_id, system_type_id, user_type_id, CONVERT(nvarchar(128), start_value) AS start_value, CONVERT(nvarchar(128), increment) AS increment, CONVERT(nvarchar(128), minimum_value) AS minimum_value, CONVERT(nvarchar(128), maximum_value) AS maximum_value, CONVERT(nvarchar(128), current_value) AS current_value, is_cycling, is_cached, cache_size FROM sys.sequences WHERE is_ms_shipped = 0 FOR JSON PATH)) AS sequences,
    JSON_QUERY((SELECT * FROM sys.sql_expression_dependencies FOR JSON PATH)) AS dependencies,
    JSON_QUERY((SELECT * FROM sys.xml_schema_collections WHERE xml_collection_id > 1 FOR JSON PATH)) AS xml_schema_collections,
    JSON_QUERY((SELECT * FROM sys.database_principals FOR JSON PATH)) AS principals,
    JSON_QUERY((SELECT * FROM sys.database_permissions FOR JSON PATH)) AS permissions,
    JSON_QUERY((SELECT * FROM sys.database_role_members FOR JSON PATH)) AS role_members,
    JSON_QUERY((SELECT class, major_id, minor_id, name, CONVERT(nvarchar(max), value) AS value FROM sys.extended_properties FOR JSON PATH)) AS extended_properties,
    JSON_QUERY((SELECT SCHEMA_NAME(t.schema_id) AS schema_name, t.name AS table_name, SUM(p.rows) AS approximate_rows FROM sys.tables t JOIN sys.partitions p ON p.object_id = t.object_id AND p.index_id IN (0, 1) WHERE t.is_ms_shipped = 0 GROUP BY t.schema_id, t.name FOR JSON PATH)) AS row_counts
FOR JSON PATH, WITHOUT_ARRAY_WRAPPER;



