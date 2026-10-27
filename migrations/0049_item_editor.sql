-- Item editor of the admin panel (docs/future/item-editor.md, cut 2), through the content edits:
-- item: an added item (target = its ItemClass; value = template, set columns, text, costumes,
--   assets, set_id), new or changed;
-- item_override: editable columns and text of a base item (heroesContents.db3 is never changed);
-- item_hide: {"hidden": true|false}, the item leaves (or comes back to) the NPC shops and the
--   panel's give list, and the items characters own keep working;
-- item_remove: an added item leaves the content (refused while characters own one).
ALTER TABLE content_edits DROP CONSTRAINT content_edits_kind_check;
ALTER TABLE content_edits ADD CONSTRAINT content_edits_kind_check CHECK (kind IN (
    'shop_row', 'shop_row_remove', 'cash_price', 'cash_default',
    'feature', 'quest_drop', 'enhance_ratio', 'enchant_ratio',
    'item', 'item_override', 'item_hide', 'item_remove'));
CREATE INDEX content_edits_item_target ON content_edits (target, id)
    WHERE kind IN ('item', 'item_override', 'item_hide', 'item_remove');
-- Which characters own an item (the editor warns before hiding and refuses removal).
CREATE INDEX game_items_item_class ON game_items (item_class_utf16);
