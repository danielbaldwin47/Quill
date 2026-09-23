#!/usr/bin/env python3
"""Type through the kernel, and say exactly when each key was pressed.

Owner: latency piece. Used by tools/bench.mjs and dev/legacy/tools/latency.mjs --uinput.

Chromium's CDP `Input.dispatchKeyEvent` starts the clock *inside the browser process*: the whole
kernel -> libinput -> compositor -> client hop is excluded, so a keyboard-to-photon number built
on it is optimistic by an unknown few milliseconds. This creates a real virtual keyboard on
/dev/uinput, so the keys travel evdev -> libinput -> Hyprland -> Wayland -> the client exactly as
the user's own keyboard does, and records CLOCK_MONOTONIC immediately before each write(2).

Chromium's TimeTicks are CLOCK_MONOTONIC on Linux, and its trace timestamps are in the same
microseconds, so `trace_keydown_ts - t_ns/1000` is the delivery hop, measured, in one clock. GTK's
event times and `GdkFrameTimings` are that clock again, which is what tools/bench.mjs joins on.

stdin  : line 1, the plan:
         {"pace_ms":90,"hold_ms":12,"settle_ms":1500,"chunk":25,"text":"...",
          "keys":[{"press":"a"},{"press":"Control+z"},{"press":"Shift+ArrowLeft"},
                  {"press":"e","pause_ms":1400}]}
         `text` is a convenience: one key per character, converted with the US layout table below.
         `keys` wins when both are given, and is the only form that can spell a chord or ask for a
         longer wait after one key. `pause_ms` replaces that key's `pace_ms`, it is not added to it.
         Then one line per chunk: "go" types the next `chunk` keys, "go N" the next N, "end" stops.
stdout : line 1 {"ready":true,...}; one {"chunk":i,"typed":n} per "go"; then the summary
         {"ok":true,"n":300,"clock":"CLOCK_MONOTONIC","events":[{"i","code","shift","t_ns"},...]}.

WHY IT IS CHUNKED. Real keys go wherever the compositor thinks focus is, and this machine has
terminals on it. The caller re-verifies, between every chunk, that the window still reports keyboard
focus; if it does not it sends "end" and nothing more is typed. A single write of 300 keys could
not be stopped half way.

WHY A CHORD IS ONE KEY. `Control+z` is two keydowns and the app sees both, but it is one thing the
writer did and one thing to be timed. The modifier goes down in the same packet as the key, the
write(2) is stamped once, and one event is recorded — so the i-th event is the i-th non-modifier
keydown, and the modifier is a keydown nobody wrote. That is the accounting the legacy bench's
uinput mode kept for the Shift of a capital letter, and a chord is the same shape.
"""
import ctypes, fcntl, json, os, struct, sys, time

UINPUT = '/dev/uinput'
EV_SYN, EV_KEY = 0x00, 0x01
SYN_REPORT = 0
UI_SET_EVBIT, UI_SET_KEYBIT = 0x40045564, 0x40045565
UI_DEV_CREATE, UI_DEV_DESTROY = 0x5501, 0x5502

# US layout, the only one this machine has configured (hyprctl getoption input:kb_layout -> "us").
ROW = {'1': 2, '2': 3, '3': 4, '4': 5, '5': 6, '6': 7, '7': 8, '8': 9, '9': 10, '0': 11,
       '-': 12, '=': 13, '[': 26, ']': 27, ';': 39, "'": 40, '`': 41, '\\': 43,
       ',': 51, '.': 52, '/': 53, ' ': 57}
LETTERS = dict(zip('qwertyuiop', range(16, 26)))
LETTERS.update(dict(zip('asdfghjkl', range(30, 39))))
LETTERS.update(dict(zip('zxcvbnm', range(44, 51))))
SHIFTED = {'!': '1', '@': '2', '#': '3', '$': '4', '%': '5', '^': '6', '&': '7', '*': '8',
           '(': '9', ')': '0', '_': '-', '+': '=', '{': '[', '}': ']', ':': ';', '"': "'",
           '~': '`', '|': '\\', '<': ',', '>': '.', '?': '/'}
KEY_ENTER, KEY_BACKSPACE, KEY_LEFTSHIFT, KEY_LEFTCTRL = 28, 14, 42, 29
# The keys a press can name instead of spelling as a character. The regimes reach for ArrowLeft
# only; the rest of the block is here because a keyboard with one arrow key has all four.
NAMED = {'Enter': KEY_ENTER, 'Backspace': KEY_BACKSPACE, 'Space': 57, 'Tab': 15,
         'ArrowLeft': 105, 'ArrowRight': 106, 'ArrowUp': 103, 'ArrowDown': 108,
         'Home': 102, 'End': 107}
# The modifiers a chord may hold. Left-hand keys, because that is the half a keyboard has one of.
MODIFIERS = {'Shift': KEY_LEFTSHIFT, 'Control': KEY_LEFTCTRL}

def code_for(ch):
    if ch == '\n': return (KEY_ENTER, 0)
    if ch == '\b': return (KEY_BACKSPACE, 0)
    if ch in LETTERS: return (LETTERS[ch], 0)
    if ch.lower() in LETTERS and ch.isupper(): return (LETTERS[ch.lower()], 1)
    if ch in ROW: return (ROW[ch], 0)
    if ch in SHIFTED: return (ROW[SHIFTED[ch]], 1)
    return None

def press_for(press):
    """One press spelling -> (code, [modifier codes]), or None when this keyboard cannot say it.

    A one-character spelling is that character, checked first so that a literal '+' is a key and
    not an empty chord. Anything longer is `Modifier+...+Key`, where the key is either a character
    or one of NAMED. A capital or a shifted character brings its own Shift, so `Control+A` holds
    both, which is what a hand does.
    """
    if len(press) == 1:
        c = code_for(press)
        return None if c is None else (c[0], [KEY_LEFTSHIFT] if c[1] else [])
    parts = press.split('+')
    if len(parts) < 2: return None
    mods = []
    for name in parts[:-1]:
        if name not in MODIFIERS: return None
        mods.append(MODIFIERS[name])
    tail = parts[-1]
    if tail in NAMED: return (NAMED[tail], list(dict.fromkeys(mods)))
    if len(tail) != 1: return None
    c = code_for(tail)
    if c is None: return None
    if c[1]: mods.append(KEY_LEFTSHIFT)
    return (c[0], list(dict.fromkeys(mods)))

def resolve(plan):
    """A plan's keys as codes and modifiers, or the press this keyboard cannot say."""
    asked = plan.get('keys')
    if asked is None:
        asked = [{'press': ch} for ch in plan.get('text', '')]
    keys = []
    for k in asked:
        if 'press' in k:
            r = press_for(k['press'])
            if r is None: return None, k['press']
            code, mods = r
        else:                                 # the older form: a resolved code and a Shift flag
            code, mods = k['code'], ([KEY_LEFTSHIFT] if k.get('shift') else [])
        keys.append({'code': code, 'mods': mods, 'pause_ms': k.get('pause_ms')})
    return keys, None

def main():
    plan = json.loads(sys.stdin.readline())   # ONE line: stdin stays open for the chunk commands
    keys, bad = resolve(plan)
    if bad is not None: json.dump({'ok': False, 'error': 'no key for %r' % bad}, sys.stdout); return 2
    # A reused device types plan after plan: each ends in a newline-framed summary, and the next
    # line is either the next plan or the end of stdin. So it declares every key it could be asked
    # for rather than the first plan's, and settles once for the whole run.
    reuse = bool(plan.get('reuse'))
    pace = plan.get('pace_ms', 90) / 1000.0
    hold = plan.get('hold_ms', 12) / 1000.0
    settle = plan.get('settle_ms', 1500) / 1000.0
    chunk = int(plan.get('chunk', 0)) or len(keys)

    fd = os.open(UINPUT, os.O_WRONLY | os.O_NONBLOCK)
    for bit in (EV_KEY, EV_SYN): fcntl.ioctl(fd, UI_SET_EVBIT, bit)
    # Every code the plan will press, its modifiers among them: a key the device never declared is
    # a key the kernel drops, and a chord whose Control was never declared types a bare letter.
    wanted = sorted({k['code'] for k in keys} | {m for k in keys for m in k['mods']} | {KEY_LEFTSHIFT})
    if reuse:
        wanted = sorted(set(ROW.values()) | set(LETTERS.values()) | set(NAMED.values())
                        | set(MODIFIERS.values()))
    for c in wanted: fcntl.ioctl(fd, UI_SET_KEYBIT, c)
    # struct uinput_user_dev: char name[80]; struct input_id id; __u32 ff_effects_max; 4 x __s32[64]
    dev = struct.pack('=80sHHHHi' + '64i' * 4, b'quill-latency-bench',
                      0x03, 0x1209, 0x0001, 0x0001, 0, *([0] * 256))
    os.write(fd, dev)
    fcntl.ioctl(fd, UI_DEV_CREATE)
    time.sleep(settle)                       # let the compositor pick the new keyboard up

    # struct input_event { struct timeval time; __u16 type, code; __s32 value; } — and on 64-bit
    # Linux a timeval is two 8-byte longs, so the record is 24 bytes. '=llHHi' is 16: in struct's
    # STANDARD size mode 'l' is 4 bytes whatever the platform. The kernel then reads `type` out of
    # what is really the next event's timestamp, sees 0 (EV_SYN), and every key press is silently
    # swallowed as a bare SYN_REPORT — a device that enumerates perfectly and delivers nothing.
    # That is why round 3's uinput run "never ran": it could not have worked. '@' is native.
    EVENT = struct.Struct('@llHHi')
    assert EVENT.size == 24, 'struct input_event must be 24 bytes here, got %d' % EVENT.size
    def ev(t, c, v):                         # time 0 -> the kernel stamps it itself
        return EVENT.pack(0, 0, t, c, v)

    def summary(out, keys):
        return {'ok': True, 'n': len(out), 'requested': len(keys), 'clock': 'CLOCK_MONOTONIC',
                'device': 'quill-latency-bench (uinput, bus USB)', 'events': out}

    try:
        while True:
            out = type_plan(fd, ev, keys, pace, hold, chunk)
            if not reuse: break
            print(json.dumps(summary(out, keys)), flush=True)
            line = sys.stdin.readline()
            if not line.strip(): return 0
            plan = json.loads(line)
            keys, bad = resolve(plan)
            if bad is not None: print(json.dumps({'ok': False, 'error': 'no key for %r' % bad}), flush=True); return 2
            pace = plan.get('pace_ms', 90) / 1000.0
            hold = plan.get('hold_ms', 12) / 1000.0
            chunk = int(plan.get('chunk', 0)) or len(keys)
    finally:
        time.sleep(0.2)
        fcntl.ioctl(fd, UI_DEV_DESTROY)
        os.close(fd)
    json.dump(summary(out, keys), sys.stdout)
    sys.stdout.flush()
    return 0

def type_plan(fd, ev, keys, pace, hold, chunk):
    """Types one plan chunk by chunk, as the caller asks, and answers with what it wrote."""
    out = []
    print(json.dumps({'ready': True, 'keys': len(keys), 'chunk': chunk}), flush=True)
    if True:
        i = 0
        while i < len(keys):
            cmd = sys.stdin.readline().split()
            if not cmd or cmd[0] != 'go':         # "end", EOF, or anything unexpected: stop typing
                break
            # "go N" types N rather than a chunk: the bench's warm-up ends on one, so the measured
            # keys go through the same device without a second settle.
            asked_now = int(cmd[1]) if len(cmd) > 1 else chunk
            first = i
            for _ in range(min(asked_now, len(keys) - i)):
                k = keys[i]
                pkt = b''
                for m in k['mods']: pkt += ev(EV_KEY, m, 1)
                pkt += ev(EV_KEY, k['code'], 1) + ev(EV_SYN, SYN_REPORT, 0)
                t = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
                os.write(fd, pkt)
                out.append({'i': i, 'code': k['code'], 'shift': int(KEY_LEFTSHIFT in k['mods']), 't_ns': t})
                time.sleep(hold)
                up = ev(EV_KEY, k['code'], 0)
                for m in reversed(k['mods']): up += ev(EV_KEY, m, 0)
                os.write(fd, up + ev(EV_SYN, SYN_REPORT, 0))
                # A regime with pauses waits here instead of at the pace: a burst is 25 keys and
                # then the writer thinking, and the first key after the thinking is the one the
                # regime exists to time.
                wait = k['pause_ms'] / 1000.0 if k['pause_ms'] is not None else pace
                rest = wait - hold
                if rest > 0: time.sleep(rest)
                i += 1
            print(json.dumps({'chunk': first, 'typed': i - first}), flush=True)
    return out

if __name__ == '__main__':
    sys.exit(main())
