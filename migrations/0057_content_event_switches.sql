-- Event switches of the panel (laboratory extension, Ajustes de jogo → Eventos): content edits
-- of kind 'event' (target = the FeatureMatrix key; value = {"enabled": true|false}), published
-- with the tuning and applied to the server's policies and the client's SyncFeatureMatrix.
ALTER TABLE content_edits DROP CONSTRAINT content_edits_kind_check;
ALTER TABLE content_edits ADD CONSTRAINT content_edits_kind_check CHECK (kind IN (
    'shop_row', 'shop_row_remove', 'cash_price', 'cash_default',
    'feature', 'quest_drop', 'enhance_ratio', 'enchant_ratio',
    'item', 'item_override', 'item_hide', 'item_remove', 'event'));
