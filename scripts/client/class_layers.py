"""Class layers of the client versions the launcher distributes (docs/future/game-launcher.md 4.7).

The versions are cumulative: v1 has no new class, v2 adds the Delia (class 10), v3 the Miri (class
11). Every script that patches or fills the client for a class takes `--classes none|delia|delia,miri`
(default: every class, the output of before the layers); a layer is always a prefix of LAYERS.
"""
from __future__ import annotations
import argparse

LAYERS = ('delia', 'miri')
INDEX = {'delia': 10, 'miri': 11}
VERSIONS = {1: (), 2: ('delia',), 3: ('delia', 'miri')}


def parse(text: str) -> tuple[str, ...]:
    """'none', 'delia' or 'delia,miri' (case and spaces ignored) -> the class keys."""
    value = text.strip().lower()
    keys = () if value in ('', 'none') else tuple(k.strip() for k in value.split(','))
    if keys != LAYERS[:len(keys)]:
        raise argparse.ArgumentTypeError(f'--classes must be none, {", ".join(",".join(LAYERS[:n]) for n in range(1, len(LAYERS) + 1))}')
    return keys


def indices(keys: tuple[str, ...]) -> tuple[int, ...]:
    return tuple(INDEX[k] for k in keys)


def add_argument(parser: argparse.ArgumentParser, allow_none: bool = True) -> None:
    choices = 'none|' if allow_none else ''
    parser.add_argument('--classes', type=parse, default=LAYERS,
                        help=f'class layer ({choices}delia|delia,miri; default: every class)')


def checked(keys: tuple[str, ...], allow_none: bool = True) -> tuple[str, ...]:
    keys = parse(','.join(keys) if keys else 'none')
    if not keys and not allow_none:
        raise ValueError('This patch needs at least one class; skip it for a version without new classes.')
    return keys
