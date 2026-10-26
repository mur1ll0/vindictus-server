-- Game tuning through the panel's content edits (B4.5), applied when the game server starts:
-- feature: a FeatureMatrix rate (ExpRate/GoldRate/APRate of combat and story, ExpRate_Monster);
-- quest_drop: QuestInfo DropItemCountMin/Max of a quest;
-- enhance_ratio: EnhanceServerInfo SuccessRatio of "<EnhanceType>#<EnhanceLevel>";
-- enchant_ratio: EnchantInfo Min/MaxSuccessRatio of an EnchantClass.
ALTER TABLE content_edits DROP CONSTRAINT content_edits_kind_check;
ALTER TABLE content_edits ADD CONSTRAINT content_edits_kind_check CHECK (kind IN (
    'shop_row', 'shop_row_remove', 'cash_price', 'cash_default',
    'feature', 'quest_drop', 'enhance_ratio', 'enchant_ratio'));
