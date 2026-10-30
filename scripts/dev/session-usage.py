"""Resume os tokens gastos por uma sessão do Claude Code a partir do transcript local.

Uso: py -3.13 scripts/dev/session-usage.py [<session-id>]
Sem argumento, usa o transcript modificado mais recentemente deste projeto.
Soma a sessão principal e os subagentes (<session-id>/subagents/*.jsonl), contando
cada mensagem da API uma única vez (os registros de streaming repetem o message.id).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

FIELDS = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens")


def project_dir() -> Path:
    root = Path(__file__).resolve().parents[2]
    slug = "".join(c if c.isalnum() else "-" for c in str(root))
    return Path.home() / ".claude" / "projects" / slug


def usage_of(path: Path) -> tuple[dict[str, int], int, int]:
    """Totais por campo, número de chamadas e contexto da última chamada."""
    seen: dict[str, dict[str, int]] = {}
    last_context = 0
    with path.open(encoding="utf-8") as lines:
        for line in lines:
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            message = record.get("message")
            if not isinstance(message, dict) or not isinstance(message.get("usage"), dict):
                continue
            usage = {field: int(message["usage"].get(field) or 0) for field in FIELDS}
            seen[message.get("id") or f"line-{len(seen)}"] = usage
            last_context = usage["input_tokens"] + usage["cache_creation_input_tokens"] + usage["cache_read_input_tokens"]
    totals = {field: sum(u[field] for u in seen.values()) for field in FIELDS}
    return totals, len(seen), last_context


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    folder = project_dir()
    if len(sys.argv) > 1:
        main_log = folder / f"{sys.argv[1]}.jsonl"
    else:
        logs = sorted(folder.glob("*.jsonl"), key=lambda p: p.stat().st_mtime)
        if not logs:
            print(f"nenhum transcript em {folder}", file=sys.stderr)
            return 1
        main_log = logs[-1]
    if not main_log.is_file():
        print(f"transcript ausente: {main_log}", file=sys.stderr)
        return 1

    totals, calls, context = usage_of(main_log)
    for sub in sorted((folder / main_log.stem / "subagents").glob("*.jsonl")):
        sub_totals, sub_calls, _ = usage_of(sub)
        calls += sub_calls
        for field in FIELDS:
            totals[field] += sub_totals[field]

    total = sum(totals.values())
    print(f"sessão {main_log.stem}: {calls} chamadas à API")
    print(f"  entrada {totals['input_tokens']:,} | cache criado {totals['cache_creation_input_tokens']:,}"
          f" | cache lido {totals['cache_read_input_tokens']:,} | saída {totals['output_tokens']:,}")
    print(f"  total processado {total:,} tokens; contexto da última chamada {context:,} tokens")
    return 0


if __name__ == "__main__":
    sys.exit(main())
