"""Export structural inventory contracts only; no rows or proprietary definitions."""
from pathlib import Path
import hashlib,json
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
path=Path("docs/evidence/database/study_heroes.schema.json")
d=json.loads(path.read_text(encoding="utf-8-sig"))
table_names={t["object_id"]:t["name"] for t in d["tables"]}
column_names={(c["object_id"],c["column_id"]):c["name"] for c in d["columns"]}
names=("Item","ItemAttribute","ItemCollection","ItemStorageStatus","Equippable")
result=[]
for name in names:
 table=next(t for t in d["tables"] if t["name"]==name);tid=table["object_id"]
 columns=[{key:c.get(key) for key in ("name","declared_type","max_length","precision","scale","is_nullable","collation_name")} for c in d["columns"] if c["object_id"]==tid]
 fks=[]
 for f in d["foreign_keys"]:
  if not f or f["parent_object_id"]!=tid:continue
  pairs=[{"column":column_names[(tid,p["parent_column_id"])],"referenced_column":column_names[(f["referenced_object_id"],p["referenced_column_id"])]} for p in d["foreign_key_columns"] if p["constraint_object_id"]==f["object_id"]]
  fks.append({"references":table_names[f["referenced_object_id"]],"columns":pairs,"delete":f["delete_referential_action_desc"],"update":f["update_referential_action_desc"],"disabled":f["is_disabled"],"not_trusted":f["is_not_trusted"]})
 identities=[column_names[(tid,i["column_id"])] for i in d["identity_columns"] if i["object_id"]==tid]
 result.append({"table":name,"column_count":len(columns),"columns":columns,"foreign_keys":fks,"identity_columns":identities})
base="legacy/decompiled/PlayerService-f23d8b9df77c/"
paths=["PlayerService.ItemServiceCore/ItemContents.cs","PlayerService.ItemServiceCore.TradeSystem/ItemGenerator.cs","PlayerService.ItemServiceCore.ItemInstance/Item.cs","PlayerService.ItemServiceCore.ItemInstance/ItemLocation.cs","PlayerService.ItemServiceCore/ItemClassConst.cs"]
e={"stage":"M13.2d.2b.2c.2a","catalog_sha256":sha(path),"tables":result,
"sources":[{"path":base+p,"sha256":sha(base+p)} for p in paths],
"catalog_lookup":{"null_returns_none":True,"ordinal_keys":True,"filter_order":["FeatureMatrix.IsEnable(row.Feature)","row.IsValid()"],"duplicate_keys":"last row overwrites","additional_initialization_pending":True},
"system_item_lookup":["ID equality SingleOrDefault","Item.IsValid()","ItemLocation.IsNull"],
"mobile_binding_order":["null info -> true","explicit bound -> true","not equippable and not gold -> true","Bind feature -> true","TradeOnBind feature -> true","enchant_scroll prefix -> Macro_EnchantTradeRestriction","CheckHighLevelEquipment"],
"high_level_dependencies":["IsEquippable","culture-specific ToLower of special cash class","ENHANCE Convert.ToInt32(value) >=9","LOOK exists","QUALITY.Arg >=5","PREFIX/SUFFIX enchant level >=7 (missing content may throw)","BraceletMaxSlotCount and GemstoneTypes"],
"snapshots_imported":False,"sqlserver_writes":False,"delivery_implemented":False}
Path("docs/evidence/auth/inventory-contract.json").write_text(json.dumps(e,ensure_ascii=True,indent=2)+"\n",encoding="utf-8",newline="\n")
print("Inventory contract: 5 tables/31 columns; source FK/actions/identity metadata; no rows/writes")
