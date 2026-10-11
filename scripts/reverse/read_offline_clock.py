"""Read only selected public configuration from a private offline SYSTEM hive."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import struct
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".tools/registry-reader"))
from Registry import Registry

def main():
    hive = ROOT / ".cache/private-user-import/SYSTEM"
    data = hive.read_bytes()
    before = hashlib.sha256(data).hexdigest()
    registry = Registry.Registry(str(hive))
    current = registry.open("Select").value("Current").value()
    control = f"ControlSet{current:03d}"
    zone = registry.open(control + r"\Control\TimeZoneInformation")
    public_values = {}
    for name in ["TimeZoneKeyName", "StandardName", "DaylightName", "Bias", "StandardBias",
                 "DaylightBias", "ActiveTimeBias", "DynamicDaylightTimeDisabled", "StandardStart", "DaylightStart"]:
        try:
            value = zone.value(name).value()
        except Registry.RegistryValueNotFoundException:
            value = None
        if isinstance(value, bytes):
            value = value.hex().upper()
        if name.endswith("Bias") and isinstance(value, int) and value >= 2**31:
            value -= 2**32
        public_values[name] = value
    services = registry.open(control + r"\Services")
    sql_present = any(k.name() == "MSSQLSERVER" or k.name().startswith("MSSQL$") for k in services.subkeys())
    sequences = struct.unpack_from("<II", data, 4)
    checksum = 0
    for word in struct.unpack("<127I", data[:0x1FC]):
        checksum ^= word
    if checksum == 0:
        checksum = 1
    elif checksum == 0xFFFFFFFF:
        checksum = 0xFFFFFFFE
    header_valid = checksum == struct.unpack_from("<I", data, 0x1FC)[0]
    report = {
        "inspected_at_utc": datetime.now(timezone.utc).isoformat(),
        "method": "Read-only python-registry parser of private VMDK SYSTEM copy; only Select/time zone and presence of SQL service; VM never started",
        "source_disk": "Vindictus1.69EN/VinServer/VinServer 2008 x64.vmdk",
        "source_archive_path": "Windows/System32/config/SYSTEM",
        "hive_sha256_before": before,
        "hive_sha256_after": hashlib.sha256(hive.read_bytes()).hexdigest(),
        "hive_sequence_numbers": list(sequences),
        "hive_sequences_match": sequences[0] == sequences[1],
        "hive_header_checksum_valid": header_valid,
        "transaction_logs_replayed": False,
        "native_hive_load_attempt_win32_error": 1009,
        "active_control_set": control,
        "time_zone": public_values,
        "sql_service_present": sql_present,
        "original_server_binary_profile_matched": False,
        "current_registry_modified": False,
        "parser": {"name": "python-registry", "version": "1.3.1", "license": "Apache-2.0",
                   "source": "https://github.com/williballenthin/python-registry"},
    }
    if report["hive_sha256_before"] != report["hive_sha256_after"]:
        raise ValueError("Private hive changed during read")
    target = ROOT / "docs/evidence/database/offline-vm-clock.json"
    vm_binary = ROOT / ".cache/private-user-import/LoginServiceCore.dll"
    reference_binary = ROOT.parent / "ServidorVindictus/Server/zh-tw-x-gm/Bin/LoginServiceCore.dll"
    if vm_binary.is_file() and reference_binary.is_file():
        vm_hash = hashlib.sha256(vm_binary.read_bytes()).hexdigest()
        reference_hash = hashlib.sha256(reference_binary.read_bytes()).hexdigest()
        report.update({
            "original_server_binary_profile_matched": vm_hash == reference_hash,
            "login_assembly_sha256": vm_hash,
            "recovered_login_assembly_sha256": reference_hash,
            "login_archive_path": "Server/zh-tw-x-gm/Bin/LoginServiceCore.dll",
        })
    zero_dst = public_values["StandardStart"] == "0" * 32 and public_values["DaylightStart"] == "0" * 32
    report["daylight_transitions_zero"] = zero_dst
    report["inferred_fixed_offset_minutes"] = -(public_values["Bias"] + public_values["StandardBias"]) if zero_dst else None
    if not header_valid or sequences[0] != sequences[1]:
        raise ValueError("Offline hive requires recovery; clock profile is not verified")
    target.write_text(json.dumps(report, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    print(f"Offline VM clock: name={public_values['TimeZoneKeyName']}; bias={public_values['Bias']}; SQL service present={sql_present}; matched original binaries={report['original_server_binary_profile_matched']}; hive sequences match={report['hive_sequences_match']}")

if __name__ == "__main__":
    main()
