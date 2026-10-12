"""Read selected regional settings from private, offline registry copies."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib,json,struct,sys
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/".tools/registry-reader"))
from Registry import Registry

def main():
    private=ROOT/".cache/private-user-import"
    source_paths=json.loads((private/"hive-paths.json").read_text(encoding="utf-8"))
    profiles=[]
    names=["Locale","LocaleName","sShortDate","sDate","sTime","sTimeFormat","s1159","s2359","iCalendarType"]
    for index,source in enumerate(source_paths):
        path=private/f"culture-{index}.hive"
        data=path.read_bytes()
        digest=hashlib.sha256(data).hexdigest()
        seq=struct.unpack_from("<II",data,4)
        checksum=0
        for word in struct.unpack("<127I",data[:0x1fc]):checksum^=word
        checksum=1 if checksum==0 else 0xfffffffe if checksum==0xffffffff else checksum
        if seq[0]!=seq[1] or checksum!=struct.unpack_from("<I",data,0x1fc)[0]:
            raise ValueError("Offline regional hive requires recovery")
        reg=Registry.Registry(str(path))
        values=None
        try:
            key=reg.open(r"Control Panel\International")
            values={}
            for name in names:
                try:values[name]=key.value(name).value()
                except Registry.RegistryValueNotFoundException:values[name]=None
        except Registry.RegistryKeyNotFoundException:pass
        assert hashlib.sha256(path.read_bytes()).hexdigest()==digest
        profiles.append({"profile_index":index,"private_archive_path_sha256":hashlib.sha256(source.encode()).hexdigest(),
            "hive_sha256":digest,"sequences_match":True,"header_checksum_valid":True,"regional_values":values})
    configured=[p["regional_values"] for p in profiles if p["regional_values"] is not None]
    assert len(configured)==5 and all(v==configured[0] for v in configured)
    assert configured[0]["LocaleName"]=="en-US" and configured[0]["iCalendarType"]=="1"
    zone=json.loads((ROOT/"docs/evidence/database/offline-vm-clock.json").read_text(encoding="utf-8"))
    assert zone["original_server_binary_profile_matched"] and zone["inferred_fixed_offset_minutes"]==480
    assert hashlib.sha256((private/"SYSTEM").read_bytes()).hexdigest()==zone["hive_sha256_before"]
    bundle=private.parent/"private-character-import/bundle.json"
    rows=json.loads(bundle.read_text(encoding="utf-8-sig"))["queued_items"]
    arguments=0
    attributes=0
    for row in rows:
        text=bytes.fromhex(row["item_class_ex_utf16"]).decode("utf-16-le",errors="surrogatepass")
        attributes+=("[" in text)
        arguments+=text.count("(")+text.count("<")
    e={"inspected_at_utc":datetime.now(timezone.utc).isoformat(),
        "method":"Read-only python-registry on six private NTUSER copies; selected regional values only; VM never started",
        "profiles":profiles,"configured_profiles":len(configured),"regional_values_agree":True,
        "selected_profile":"en-US Gregorian, fixed UTC+8, explicit selection only",
        "runtime_user_or_thread_culture_observed":False,"user_override_scope":"selected date/time settings; general NLS collation not proven",
        "source_queue_census":{"rows":len(rows),"with_attributes":attributes,"argument_delimiters":arguments,
            "private_snapshot_sha256":hashlib.sha256(bundle.read_bytes()).hexdigest(),"rows_published":False},
        "zone_evidence_path":"docs/evidence/database/offline-vm-clock.json",
        "zone_evidence_sha256":hashlib.sha256((ROOT/"docs/evidence/database/offline-vm-clock.json").read_bytes()).hexdigest(),
        "source_services_executed":False,"source_databases_written":False,"current_registry_modified":False}
    (ROOT/"docs/evidence/auth/item-temporal-profile.json").write_text(json.dumps(e,ensure_ascii=True,indent=2)+"\n",encoding="utf-8",newline="\n")
    print(f"Offline profile PASS: hives={len(profiles)}, configured={len(configured)}, consensus=en-US, fixed_zone=UTC+8; queue_rows={len(rows)}, attributes={attributes}, arguments={arguments}; runtime culture not observed")
if __name__=="__main__":main()
