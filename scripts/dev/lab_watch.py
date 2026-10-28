"""Catch writes to an address in the running laboratory client with a hardware breakpoint.

Attaches as a debugger (needs an administrator terminal, like the client), sets a write
breakpoint in the debug registers of every thread (DR0-DR3, one dword each), and for each write
records the thread, the instruction after the write and the return addresses found on the stack
inside client.dll, server.dll and engine.dll. The game's memory is never written; on exit the
debug registers are cleared and the debugger detaches without killing the process.
Observed (D8o): the attach and detach went through, but the client aborted shortly after detaching
("Microsoft Visual C++ Runtime Library: Runtime Error ... terminate it in an unusual way"), most
likely the anti-cheat reacting to the debugger. Expect to restart the client after each use.

Usage: python -s -E scripts/dev/lab_watch.py EXPR [EXPR ...] [--hits N] [--seconds S]
EXPR is an address or a lab_memory expression (`[[0x40cb3740]+0x14]+0x3418`); `EXPR*N` watches N
consecutive dwords (at most four in all).
Output: printed and written to .cache/lab-memory/watch.txt as it happens.
"""
from __future__ import annotations
import argparse
import ctypes
import ctypes.wintypes as wt
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lab_memory  # noqa: E402

OUTPUT = lab_memory.CHANNEL / 'watch.txt'
kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
kernel32.OpenThread.restype = wt.HANDLE
kernel32.DebugActiveProcess.argtypes = [wt.DWORD]
kernel32.DebugActiveProcessStop.argtypes = [wt.DWORD]
kernel32.DebugSetProcessKillOnExit.argtypes = [wt.BOOL]
kernel32.WaitForDebugEvent.argtypes = [ctypes.c_void_p, wt.DWORD]
kernel32.ContinueDebugEvent.argtypes = [wt.DWORD, wt.DWORD, wt.DWORD]
kernel32.Wow64GetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]
kernel32.Wow64SetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]

DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED = 0x00010002, 0x80010001
EXCEPTION_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, CREATE_PROCESS_DEBUG_EVENT = 1, 2, 3
EXIT_PROCESS_DEBUG_EVENT, LOAD_DLL_DEBUG_EVENT = 5, 6
SINGLE_STEP = (0x80000004, 0x4000001E)  # native and WOW64 (STATUS_WX86_SINGLE_STEP)
BREAKPOINT = (0x80000003, 0x4000001F)   # the attach breakpoint, native and WOW64
WOW64_CONTEXT_FLAGS = 0x10000 | 0x1 | 0x2 | 0x10  # i386 | CONTROL | INTEGER | DEBUG_REGISTERS
CONTEXT_SIZE = 716
DEBUG_EVENT_SIZE = 176
THREAD_ACCESS = 0x0008 | 0x0010 | 0x0002 | 0x0040  # GET/SET_CONTEXT, SUSPEND_RESUME, QUERY


class Context:
    """WOW64_CONTEXT as raw bytes with the few fields used here."""
    OFFSETS = {'Dr0': 4, 'Dr1': 8, 'Dr2': 12, 'Dr3': 16, 'Dr6': 20, 'Dr7': 24,
               'Edi': 156, 'Esi': 160, 'Ebx': 164, 'Edx': 168, 'Ecx': 172, 'Eax': 176,
               'Ebp': 180, 'Eip': 184, 'EFlags': 192, 'Esp': 196}

    def __init__(self):
        self.raw = ctypes.create_string_buffer(CONTEXT_SIZE)
        struct.pack_into('<I', self.raw, 0, WOW64_CONTEXT_FLAGS)

    def __getitem__(self, name):
        return struct.unpack_from('<I', self.raw, self.OFFSETS[name])[0]

    def __setitem__(self, name, value):
        struct.pack_into('<I', self.raw, self.OFFSETS[name], value & 0xFFFFFFFF)


# --exec: break before the instruction at each address instead of on writes. UNRELIABLE on the
# game (D16): the hit thread's context changes (RF, or Dr7 = 0 with TF) are not applied at the
# exception, the breakpoint re-fires and the client crashes after detach. Use write breakpoints.
EXECUTE = False


def set_breakpoints(thread: int, addresses: list[int]) -> bool:
    context = Context()
    if not kernel32.Wow64GetThreadContext(thread, context.raw):
        return False
    dr7 = 0
    for i, address in enumerate(addresses):
        context[f'Dr{i}'] = address
        dr7 |= 1 << (2 * i)                          # local enable
        if not EXECUTE:
            dr7 |= (0b01 | 0b11 << 2) << (16 + 4 * i)  # break on write, 4 bytes (execute: RW=00, LEN=00)
    context['Dr7'] = dr7
    context['Dr6'] = 0
    return bool(kernel32.Wow64SetThreadContext(thread, context.raw))


def clear_breakpoints(thread: int) -> None:
    context = Context()
    if kernel32.Wow64GetThreadContext(thread, context.raw):
        for i in range(4):
            context[f'Dr{i}'] = 0
        context['Dr6'] = context['Dr7'] = 0
        kernel32.Wow64SetThreadContext(thread, context.raw)


def describe(game, mods: dict, address: int) -> str:
    for name, (base, size, _) in mods.items():
        if base <= address < base + size and name in ('client.dll', 'server.dll', 'engine.dll'):
            return f'{name}+{address - base:#x} ({address:#x})'
    return f'{address:#x}'


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('addresses', nargs='+')
    parser.add_argument('--hits', type=int, default=8)
    parser.add_argument('--seconds', type=int, default=120)
    parser.add_argument('--follow', help='a pointer slot (EXPR) whose writes move the watch: the first '
                        'address watches the slot, the others are --then offsets from the new pointer')
    parser.add_argument('--then', default='0', help='OFFSET*N from the followed pointer (N <= 3)')
    parser.add_argument('--exec', action='store_true', help='execute breakpoints: log the registers '
                        'before the instruction at each address runs')
    args = parser.parse_args()
    global EXECUTE
    EXECUTE = args.exec
    game = lab_memory.Game()  # read handle for the stack; attaching needs the same elevation
    addresses = []
    for text in args.addresses:
        expression, _, count = text.partition('*')
        start = game.eval(expression)
        addresses += [start + 4 * i for i in range(int(count or 1))]
    follow = None
    if args.follow:
        follow = game.eval(args.follow)
        offset, _, count = args.then.partition('*')
        offset, count = int(offset, 0), min(int(count or 1), 3)
        # Only the slot at first: the entity it points to now may be freed before the respawn,
        # and writes to reused memory would use up the hit limit (D8p).
        addresses = [follow]
    addresses = addresses[:4]
    log = []
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)

    def say(text: str) -> None:
        print(text)
        log.append(text)
        OUTPUT.write_text(''.join(line + '\n' for line in log), encoding='utf-8')

    if not kernel32.DebugActiveProcess(game.pid):
        say(f'DebugActiveProcess({game.pid}) failed: error {ctypes.get_last_error()}')
        OUTPUT.write_text('\n'.join(log) + '\n', encoding='utf-8')
        return 2
    kernel32.DebugSetProcessKillOnExit(False)
    say(f'attached to {game.name} ({game.pid}); watching {", ".join(hex(a) for a in addresses)}')
    threads: dict[int, int] = {}
    event = ctypes.create_string_buffer(DEBUG_EVENT_SIZE)
    hits, deadline, armed = 0, time.time() + args.seconds, False
    stepping: set[int] = set()
    try:
        while time.time() < deadline and hits < args.hits:
            if not kernel32.WaitForDebugEvent(event, 500):
                continue
            code, pid, tid = struct.unpack_from('<III', event, 0)
            status = DBG_CONTINUE
            if code == CREATE_PROCESS_DEBUG_EVENT:
                threads[tid] = struct.unpack_from('<Q', event, 32)[0]  # hThread
            elif code == CREATE_THREAD_DEBUG_EVENT:
                threads[tid] = struct.unpack_from('<Q', event, 16)[0]
                if armed:
                    set_breakpoints(threads[tid], addresses)
            elif code == LOAD_DLL_DEBUG_EVENT:
                handle = struct.unpack_from('<Q', event, 16)[0]  # hFile
                if handle:
                    kernel32.CloseHandle(wt.HANDLE(handle))
            elif code == EXIT_PROCESS_DEBUG_EVENT:
                say('the game exited')
                break
            elif code == EXCEPTION_DEBUG_EVENT:
                exception = struct.unpack_from('<I', event, 16)[0]
                if exception in BREAKPOINT and not armed:
                    armed = True
                    done = sum(set_breakpoints(h, addresses) for h in threads.values())
                    say(f'breakpoints set on {done} of {len(threads)} threads; change the costume now')
                elif exception in SINGLE_STEP and tid in threads:
                    context = Context()
                    kernel32.Wow64GetThreadContext(threads[tid], context.raw)
                    dr6 = context['Dr6']
                    if tid in stepping and not dr6 & 0xF:
                        stepping.discard(tid)  # the stepped instruction ran: re-arm this thread
                        set_breakpoints(threads[tid], addresses)
                        kernel32.ContinueDebugEvent(pid, tid, status)
                        continue
                    which = [hex(addresses[i]) for i in range(len(addresses)) if dr6 & (1 << i)]
                    if EXECUTE:
                        hits += 1
                        regs = {r: context[r] for r in ('Eax', 'Ebx', 'Ecx', 'Edx', 'Esi', 'Edi', 'Ebp', 'Esp')}
                        say(f'hit {hits}: thread {tid} at {describe(game, game.mods, context["Eip"])} '
                            + ' '.join(f'{k.lower()}={v:#x}' for k, v in regs.items()))
                        for k, v in regs.items():
                            try:
                                text = game.string(v) if 0x10000 < v < 0x80000000 else ''
                            except Exception:
                                text = ''
                            if len(text) > 3 and text.isprintable():
                                say(f'  {k.lower()} -> {text[:120]!r}')
                        try:  # the call's first stack arguments and the object in ecx
                            args_ = struct.unpack('<4I', game.read(context['Esp'] + 4, 16))
                            say('  args ' + ' '.join(f'{a:#x}' for a in args_))
                            this = struct.unpack('<40I', game.read(context['Ecx'], 0xA0))
                            say('  this ' + ' '.join(f'{a:x}' for a in this))
                        except Exception as error:
                            say(f'  args/this unreadable: {error}')
                        # Step over: RF is not honoured for these WOW64 threads (D16 re-hit the same
                        # instruction and stalled the render thread), so disarm this thread, single-step
                        # (TF) and re-arm on the trap that follows.
                        context['Dr6'] = 0
                        context['Dr7'] = 0
                        context['EFlags'] = context['EFlags'] | 0x100
                        stepping.add(tid)
                        kernel32.Wow64SetThreadContext(threads[tid], context.raw)
                        kernel32.ContinueDebugEvent(pid, tid, status)
                        continue
                    if follow is not None and dr6 & 1:
                        target = game.dword(follow) & 0xFFFFFFFF
                        if target:
                            addresses[1:] = [target + offset + 4 * i for i in range(count)]
                            done = sum(set_breakpoints(h, addresses) for h in threads.values())
                            say(f'slot {follow:#x} now {target:#x} ({lab_memory.class_name(game, target)}); '
                                f'watching {", ".join(hex(a) for a in addresses[1:])} on {done} threads')
                        else:
                            context['Dr6'] = 0  # slot cleared: keep the old watch, just acknowledge
                            kernel32.Wow64SetThreadContext(threads[tid], context.raw)
                        kernel32.ContinueDebugEvent(pid, tid, status)
                        continue
                    if not any(b <= context['Eip'] < b + z for n, (b, z, _) in game.mods.items()
                               if n in ('client.dll', 'server.dll', 'engine.dll')):
                        say(f'  (write from outside the game DLLs at {context["Eip"]:#x}, not counted)')
                        context['Dr6'] = 0
                        kernel32.Wow64SetThreadContext(threads[tid], context.raw)
                        kernel32.ContinueDebugEvent(pid, tid, status)
                        continue
                    hits += 1
                    say(f'hit {hits}: thread {tid} wrote {", ".join(which) or "?"}; '
                        f'after {describe(game, game.mods, context["Eip"])}')
                    try:
                        stack = game.read(context['Esp'], 0x200)
                        callers = [describe(game, game.mods, v) for (v,) in struct.iter_unpack('<I', stack)
                                   if any(b <= v < b + s for n, (b, s, _) in game.mods.items()
                                          if n in ('client.dll', 'server.dll', 'engine.dll'))]
                        say('  stack: ' + ' | '.join(callers[:10]))
                    except Exception as error:  # the stack page may be unreadable
                        say(f'  stack unreadable: {error}')
                    context['Dr6'] = 0
                    kernel32.Wow64SetThreadContext(threads[tid], context.raw)
                elif exception not in BREAKPOINT:
                    status = DBG_EXCEPTION_NOT_HANDLED
            kernel32.ContinueDebugEvent(pid, tid, status)
    finally:
        for handle in threads.values():
            clear_breakpoints(handle)
        # A thread stopped mid-step still has TF set: take its trap before detaching, or the
        # single-step exception reaches the game with no debugger and kills it.
        drain = time.time() + 5
        while stepping and time.time() < drain:
            if not kernel32.WaitForDebugEvent(event, 200):
                continue
            code, pid, tid = struct.unpack_from('<III', event, 0)
            status = DBG_CONTINUE
            if code == EXCEPTION_DEBUG_EVENT:
                exception = struct.unpack_from('<I', event, 16)[0]
                if exception in SINGLE_STEP:
                    stepping.discard(tid)
                elif exception not in BREAKPOINT:
                    status = DBG_EXCEPTION_NOT_HANDLED
            kernel32.ContinueDebugEvent(pid, tid, status)
        kernel32.DebugActiveProcessStop(game.pid)
        say(f'detached after {hits} hit(s)')
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text('\n'.join(log) + '\n', encoding='utf-8')
    return 0


if __name__ == '__main__':
    sys.exit(main())
