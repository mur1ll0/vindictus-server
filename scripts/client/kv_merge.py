"""Merge the parts of an NA client script that belong to a new class into the 1.69 script.

Both files are KeyValues text (quoted or bare tokens, `//` comments, [$CONDITION] tags). The merge
walks the NA tree and, for every node whose text names the class (or its weapon), adds it to the
1.69 file when the 1.69 has nothing at that key path: a missing block is copied whole before the
closing brace of its parent, a missing key-value line likewise. A block present on both sides is
merged recursively; a key-value present on both sides with another value is a conflict, reported
and left as the 1.69 has it (for example a player_costume.txt entry number the 1.69 already uses).
Repeated keys (sound wave lists) are compared by key and value. The 1.69 bytes are never rewritten
except for the insertions, so the result is the original file plus the class's lines.
"""
from __future__ import annotations
import re
from dataclasses import dataclass, field

QUOTED = re.compile(rb'"([^"\r\n]*)"')


class MergeError(ValueError):
    pass


@dataclass
class Node:
    key: str
    value: str | None = None          # a key-value pair
    children: list['Node'] = field(default_factory=list)
    start: int = 0                    # first byte of the key (after leading comments)
    end: int = 0                      # one past the value or the closing brace
    close: int = 0                    # offset of the closing brace (blocks)

    @property
    def block(self) -> bool:
        return self.value is None


def _tokens(text: bytes):
    at, size = 0, len(text)
    while at < size:
        c = text[at]
        if c in b' \t\r\n':
            at += 1
        elif text.startswith(b'//', at):
            end = text.find(b'\n', at)
            at = size if end < 0 else end
        elif c in b'{}':
            yield chr(c), None, at, at + 1
            at += 1
        elif c == 0x22:
            end = text.find(b'"', at + 1)
            end = size if end < 0 else end
            yield 'str', text[at + 1:end].decode('latin-1'), at, end + 1
            at = end + 1
        else:
            end = at
            while end < size and text[end] not in b' \t\r\n{}"':
                end += 1
            yield 'str', text[at:end].decode('latin-1'), at, end
            at = end


def parse(text: bytes) -> Node:
    root = Node('', children=[], start=0, end=len(text), close=len(text))
    stack = [root]
    pending = None
    for kind, value, start, end in _tokens(text):
        if kind == 'str':
            if value.startswith('[$') or value.startswith('[!$'):
                continue  # platform condition after a value or a key
            if pending is None:
                pending = (value, start)
            else:
                stack[-1].children.append(Node(pending[0], value, start=pending[1], end=end))
                pending = None
        elif kind == '{':
            key, key_at = pending if pending else ('', start)
            node = Node(key, None, [], start=key_at)
            stack[-1].children.append(node)
            stack.append(node)
            pending = None
        else:
            if len(stack) == 1:
                raise MergeError('Unbalanced closing brace.')
            node = stack.pop()
            node.close, node.end = start, end
            pending = None
    if len(stack) != 1:
        raise MergeError('Unclosed block.')
    return root


def mentions(text: bytes, node: Node, needles: tuple[bytes, ...]) -> bool:
    span = text[node.start:node.end].lower()
    return any(n in span for n in needles)


def _line_start(text: bytes, at: int) -> int:
    return text.rfind(b'\n', 0, at) + 1


def _indent(text: bytes, at: int) -> bytes:
    start = _line_start(text, at)
    return re.match(rb'[ \t]*', text[start:at]).group(0)


def merge(base: bytes, source: bytes, needles: tuple[str, ...]) -> tuple[bytes, dict]:
    """Returns the 1.69 file with the class's NA nodes added, and a report (added blocks and lines,
    conflicts as key paths)."""
    wanted = tuple(n.lower().encode('latin-1') for n in needles)
    base_root, source_root = parse(base), parse(source)
    inserts: list[tuple[int, bytes]] = []
    report = {'blocks': 0, 'lines': 0, 'conflicts': []}

    def walk(base_node: Node, source_node: Node, path: tuple[str, ...]):
        by_key: dict[str, list[Node]] = {}
        for child in base_node.children:
            by_key.setdefault(child.key.lower(), []).append(child)
        repeated = {}
        for child in source_node.children:
            repeated[child.key.lower()] = repeated.get(child.key.lower(), 0) + 1
        added = []
        for child in source_node.children:
            if not mentions(source, child, wanted):
                continue
            same = by_key.get(child.key.lower(), [])
            if child.block:
                target = next((n for n in same if n.block), None)
                if target is None:
                    added.append(child)
                else:
                    walk(target, child, path + (child.key,))
            else:
                values = [n.value for n in same if not n.block]
                if child.value in values:
                    continue
                if values and len(same) == 1 and repeated[child.key.lower()] == 1:
                    report['conflicts'].append('/'.join(path + (child.key,)))
                else:
                    added.append(child)
        if not added:
            return
        # Insert before the parent's closing brace (or at the end of the file for top-level nodes),
        # each node with its own leading comment lines and the NA indentation.
        if base_node is base_root:
            at = len(base)
            prefix = b'' if base.endswith(b'\n') or not base else b'\r\n'
        else:
            at = _line_start(base, base_node.close)
            prefix = b''
        chunk = bytearray(prefix)
        for node in added:
            start = _line_start(source, node.start)
            chunk += source[start:node.end] + b'\r\n'
            report['blocks' if node.block else 'lines'] += 1
        inserts.append((at, bytes(chunk)))

    walk(base_root, source_root, ())
    out = bytearray(base)
    for at, chunk in sorted(inserts, key=lambda i: i[0], reverse=True):
        out[at:at] = chunk
    return bytes(out), report
