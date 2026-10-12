"""Publish structural metadata and hashes, never row data or recovered bodies."""
from pathlib import Path
import hashlib
import json

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
base = "legacy/decompiled/PlayerService-f23d8b9df77c/"
sources = [
    "PlayerService.ItemServiceCore.Static/QueuedItemController.cs",
    "PlayerService.ItemServiceCore/DBLoader.cs",
    "PlayerService.ItemServiceCore/ItemOwner.cs",
    "PlayerService.ItemServiceCore/ItemTransactionScope.cs",
    "PlayerService.ItemServiceCore.Static/ProcessDailyItemProcessor.cs",
    "PlayerService.ItemServiceCore._Processor/ItemFestivalProcessor.cs",
    "PlayerService.ItemServiceCore._Processor/AllUserGoalRewardProcessor.cs",
    "PlayerService.ItemServiceCore.Storage/ItemFactory.cs",
    "PlayerService.ItemServiceCore.TradeSystem/ItemGenerator.cs",
    "PlayerService.ItemServiceCore/ItemContents.cs",
    "PlayerService.ItemServiceCore.MailSystem/MailFactory.cs",
    "PlayerService.ItemServiceCore.ItemInstance/MailBox.cs",
    "PlayerService.ItemServiceCore.OperationLogSystem/ItemLogManager.cs",
    "PlayerService.ItemServiceCore/ItemLog.cs",
]
catalogs = []
for database, names in [
    ("study_heroes", ["QueuedItem", "Item", "ItemAttribute", "ItemStorageStatus", "Mail", "MailItem", "MailBox"]),
    ("study_heroesLog", ["ItemJournal", "ItemLedger", "ItemJournal01", "ItemJournal02", "ItemLedger01", "ItemLedger02"]),
]:
    path = Path(f"docs/evidence/database/{database}.schema.json")
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    tables = []
    for name in names:
        table = next(t for t in data["tables"] if t["name"] == name)
        columns = [
            {key: c.get(key) for key in ("name", "declared_type", "max_length", "precision", "scale", "is_nullable")}
            for c in data["columns"] if c["object_id"] == table["object_id"]
        ]
        tables.append({"table": name, "column_count": len(columns), "columns": columns})
    catalogs.append({"database": database, "catalog_sha256": digest(path), "tables": tables})
result = {
    "stage": "M13.2d.2b.2c.1", "scope": "selection, route/expiry decisions and dependency recovery only",
    "sources": [{"path": base+p, "sha256": digest(base+p)} for p in sources],
    "catalogs": catalogs,
    "source_services_executed": False, "database_writes": False,
    "delivery_implemented": False, "processed_updates_implemented": False,
    "pending": ["ItemClassEx parsing and item content/bind policies", "system item validation/location and destruction",
                "item allocator/factory/attributes/durability/stack/storage",
                "mail factory/attachment/box persistence and recovery queue",
                "journal/ledger routing and save/undo failure semantics",
                "atomic delivery acknowledgement and retry coordination", "frontend inventory/mail notifications"],
}
Path("docs/evidence/auth/queue-consumer-contract.json").write_text(
    json.dumps(result, ensure_ascii=True, indent=2)+"\n", encoding="utf-8", newline="\n"
)
print("Consumer contract: 14 source hashes; 13 table structures; no row data/writes")
