"""Inventory PE metadata and optionally decompile copies; never execute legacy code."""
from __future__ import annotations
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
from datetime import datetime, timezone


def pe_info(path: Path) -> dict:
    with path.open("rb") as stream:
        if stream.read(2) != b"MZ":
            return {"format": "unknown", "managed": False}
        stream.seek(0x3C)
        pe = struct.unpack("<I", stream.read(4))[0]
        stream.seek(pe)
        if stream.read(4) != b"PE\0\0":
            return {"format": "invalid-pe", "managed": False}
        machine, sections = struct.unpack("<HH", stream.read(4))
        stream.seek(pe + 24)
        magic = struct.unpack("<H", stream.read(2))[0]
        dirs = 112 if magic == 0x20B else 96
        stream.seek(pe + 24 + dirs + 14 * 8)
        clr_rva, clr_size = struct.unpack("<II", stream.read(8))
        return {"format": "PE", "machine": hex(machine), "sections": sections,
                "managed": bool(clr_rva and clr_size), "clr_header_size": clr_size}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    project = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=project.parent / "ServidorVindictus" / "Server")
    parser.add_argument("--output", type=Path, default=project / "legacy")
    parser.add_argument("--ilspy", type=Path)
    parser.add_argument("--workers", type=int, default=3)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    rows = []
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in {".exe", ".dll"}:
            continue
        try:
            metadata = pe_info(path)
        except (OSError, struct.error) as exc:
            metadata = {"managed": False, "inspection_error": str(exc)}
        rows.append({"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size,
                     "sha256": sha256(path), **metadata})
    unique = {}
    for row in rows:
        if row["managed"]:
            unique.setdefault(row["sha256"], row)
    payload = {"generated_at": datetime.now(timezone.utc).isoformat(), "source_root": str(root),
               "method": "PE headers and SHA256; ILSpy static decompilation, not execution",
               "files": rows, "binary_files": len(rows),
               "managed_files": sum(row["managed"] for row in rows),
               "unique_managed_assemblies": len(unique)}

    def save() -> None:
        (output / "inventory.json").write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    save()
    print(f"Inventory: {len(rows)} binaries, {payload['managed_files']} managed, {len(unique)} unique managed", flush=True)
    if not args.ilspy:
        return 0
    ilspy = args.ilspy.resolve(strict=True)
    logs = output / "logs"
    logs.mkdir(exist_ok=True)

    def decompile(item: tuple[str, dict]) -> dict:
        digest, row = item
        path = root / row["path"]
        target = output / "decompiled" / f"{path.stem}-{digest[:12]}"
        marker = target / ".decompiled.json"
        if marker.exists():
            cached = json.loads(marker.read_text(encoding="utf-8"))
            print(f"Cached: {path.name}", flush=True)
            return cached
        target.mkdir(parents=True, exist_ok=True)
        command = [str(ilspy), "--disable-updatecheck", "-p", "-r", str(path.parent), "-o", str(target), str(path)]
        result = {"source": row["path"], "sha256": digest, "output": target.relative_to(output).as_posix()}
        try:
            completed = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=240)
            log = completed.stdout + completed.stderr
            result["exit_code"] = completed.returncode
            result["status"] = "decompiled" if completed.returncode == 0 else "failed"
        except subprocess.TimeoutExpired:
            log = "ILSpy timed out after 240 seconds. Partial output is not a completed decompilation.\n"
            result.update(status="timeout", exit_code=None)
        (logs / f"{path.stem}-{digest[:12]}.log").write_text(log, encoding="utf-8")
        sources = list(target.rglob("*.cs"))
        result["cs_files"] = len(sources)
        result["cs_lines"] = sum(len(source.read_text(encoding="utf-8-sig", errors="replace").splitlines()) for source in sources)
        if result["status"] == "decompiled":
            marker.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(f"{result['status']}: {path.name} ({result['cs_files']} files, {result['cs_lines']} lines)", flush=True)
        return result

    priority = {"LoginServiceCore.dll": 0, "FrontendServiceCore.dll": 1, "ServiceCore.dll": 2, "Core.dll": 3}
    work = sorted(unique.items(), key=lambda item: priority.get(Path(item[1]["path"]).name, 10))
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, min(args.workers, 4))) as executor:
        futures = [executor.submit(decompile, item) for item in work]
        for future in concurrent.futures.as_completed(futures):
            results.append(future.result())
            payload["decompilation"] = sorted(results, key=lambda row: row["source"])
            save()
    failed = [row for row in results if row["status"] != "decompiled"]
    print(f"Decompiled: {len(results) - len(failed)}/{len(results)} unique assemblies", flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
