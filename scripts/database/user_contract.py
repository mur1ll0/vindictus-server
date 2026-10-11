"""Export the legacy User contract from existing catalogs and recovered source, without DB writes."""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import re

PROJECT = Path(__file__).resolve().parents[2]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    catalog_path = PROJECT / "docs/evidence/database/study_heroes.schema.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8-sig"))
    recovered = PROJECT / "legacy/decompiled/LoginServiceCore-1600fb4b593d"
    dto_path = recovered / "LoginServiceCore/User.cs"
    processor_path = recovered / "LoginServiceCore.Processor/UserLoginProcessor.cs"
    table = next(t for t in catalog["tables"] if t["name"] == "User")
    object_id = table["object_id"]
    operations = sorted(set(re.findall(r"context\.([A-Za-z_][A-Za-z_0-9]*)\(", processor_path.read_text(encoding="utf-8-sig"))) - {"SubmitChanges"})
    operations += ["GetUserBanInfo", "QueryLoginService"]
    modules = {}
    for name in operations:
        obj = next((o for o in catalog["objects"] if o["name"] == name and o["type"].strip() == "P"), None)
        definition = next((m["definition"] for m in catalog["sql_modules"] if obj and m["object_id"] == obj["object_id"]), None)
        modules[name] = {"exists_in_restored_catalog": obj is not None, "definition_sha256": hashlib.sha256(definition.encode()).hexdigest() if definition else None}
    evidence = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "method": "Static recovered-source and restored-schema comparison; no account data exported, no SQL writes",
        "source_sha256": {"catalog": digest(catalog_path), "User.cs": digest(dto_path), "UserLoginProcessor.cs": digest(processor_path)},
        "database": catalog["database_name"], "collation": catalog["collation"],
        "columns": [c for c in catalog["columns"] if c["object_id"] == object_id],
        "indexes": [i for i in catalog["indexes"] if i["object_id"] == object_id],
        "index_columns": [c for c in catalog["index_columns"] if c["object_id"] == object_id],
        "defaults": [d for d in catalog["default_constraints"] if d["parent_object_id"] == object_id],
        "dependent_modules": modules,
        "incompatibilities": [
            "User.ID signed int supplied externally, not identity/UUID",
            "Unique User.Name uses SQL_Latin1_General_CP1_CI_AS; PostgreSQL lower/citext is not full equivalence",
            "NextCharacterSN zero is CLR default, not SQL DEFAULT",
            "DTO SecondPassword NVarChar(128), restored SQL nvarchar(256)",
            "LastUpdate SQL rowversion/timestamp bytes, absent from LoginService User DTO",
            "QueryLoginService exists in recovered DataContext but is absent from restored catalog",
        ],
        "legacy_user_service_migrated": False, "game_authentication_available": False,
    }
    output = PROJECT / "docs/evidence/database/user-contract.json"
    output.write_text(json.dumps(evidence, indent=2), encoding="utf-8")
    print(f"User contract: {len(evidence['columns'])} columns; {len(modules)} module dependencies; full UserLogin remains pending.")
if __name__ == "__main__":
    main()