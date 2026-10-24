-- heroes dbo.SkillEnhance (docs/evidence/database/study_heroes.schema.json): one enhance per
-- Skill row (PK SkillRowID, no foreign key in the source), written by the
-- SkillEnhanceInsertOrUpdate/Remove/UpdateDurability/UpdateMaxDurabilityBonus procedures
-- and read by SkillEnhanceQueryAll through dbo.Skill.CID.
CREATE TABLE game_skill_enhances (
 skill_row_id BIGINT PRIMARY KEY,
 group_key INTEGER NOT NULL,
 index_key INTEGER NOT NULL,
 type SMALLINT NOT NULL CHECK(type BETWEEN 0 AND 255),
 reduce_durability INTEGER NOT NULL,
 max_durability_bonus INTEGER NOT NULL
);
