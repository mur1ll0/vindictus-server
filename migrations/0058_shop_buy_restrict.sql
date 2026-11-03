-- ShopBuyRestrict of heroes (ShopSession over the ShopBuyRestrictGet/Set/Rollback procedures,
-- study_heroes.schema.json): the bought count of a restricted shop item per character, with
-- the time of the last purchase. ShopItemID is KeyConverter.DoubleShortToInt(ShopVersion,
-- Order) for a coupon shop item. PurchaseTime (datetime, GETUTCDATE()) is kept as UTC
-- DateTime ticks, as the marketplace tables. Primary key (CID, ShopItemID) as
-- PK_ShopBuyRestrict; no foreign keys, as the source schema.
CREATE TABLE game_shop_buy_restrict(
 cid BIGINT NOT NULL,
 shop_item_id INTEGER NOT NULL,
 purchase_ticks BIGINT NOT NULL,
 count INTEGER NOT NULL,
 PRIMARY KEY(cid, shop_item_id)
);
