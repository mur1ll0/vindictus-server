-- Laboratory replacement for the Nexon NISMS billing state the CashShopService reads and
-- writes (CheckBalance, PurchaseItem, InventoryInquiry, InventoryPickup), which is not part
-- of the evidence: the cash balance of an account (NexonSN) and the orders of its character
-- inventory (GameID per CID) with the purchase attributes (colors and ItemClassEx).
CREATE TABLE game_cash_balances (
 uid INTEGER PRIMARY KEY,
 balance INTEGER NOT NULL CHECK(balance >= 0)
);
CREATE TABLE game_cash_orders (
 order_no INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 uid INTEGER NOT NULL,
 cid BIGINT NOT NULL,
 product_no INTEGER NOT NULL,
 item_class_utf16 BYTEA NOT NULL CHECK(octet_length(item_class_utf16) BETWEEN 2 AND 2048),
 pieces SMALLINT NOT NULL CHECK(pieces > 0),
 expire_days SMALLINT NOT NULL CHECK(expire_days >= 0),
 remain_quantity SMALLINT NOT NULL CHECK(remain_quantity >= 0),
 price INTEGER NOT NULL CHECK(price >= 0),
 attribute0_utf16 BYTEA CHECK(octet_length(attribute0_utf16) <= 512),
 attribute1_utf16 BYTEA CHECK(octet_length(attribute1_utf16) <= 512),
 attribute2_utf16 BYTEA CHECK(octet_length(attribute2_utf16) <= 512),
 attribute3_utf16 BYTEA CHECK(octet_length(attribute3_utf16) <= 2048)
);
CREATE INDEX game_cash_orders_open ON game_cash_orders(cid, order_no) WHERE remain_quantity > 0;
-- heroes dbo.WishList (PK_WishList on CID, ProductNo; WishListInsert refuses the 31st row).
CREATE TABLE game_cash_wishlist (
 cid BIGINT NOT NULL,
 product_no INTEGER NOT NULL,
 product_name_utf16 BYTEA CHECK(octet_length(product_name_utf16) <= 128),
 PRIMARY KEY(cid, product_no)
);
