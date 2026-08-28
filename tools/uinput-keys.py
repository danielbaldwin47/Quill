#!/usr/bin/env python3
"""Type through the kernel, and say exactly when each key was pressed.

Owner: latency piece. Used by legacy/tools/latency.mjs --uinput.

Chromium's CDP `Input.dispatchKeyEvent` starts the clock *inside the browser process*: the whole
kernel -> libinput -> compositor -> client hop is excluded, so a keyboard-to-photon number built
on it is optimistic by an unknown few milliseconds. This creates a real virtual keyboard on
/dev/uinput, so the keys travel evdev -> libinput -> Hyprland -> Wayland -> Chromium exactly as
the user's own keyboard does, and records CLOCK_MONOTONIC immediately before each write(2).

Chromium's TimeTicks are CLOCK_MONOTONIC on Linux, and its trace timestamps are in the same
microseconds, so `trace_keydown_ts - t_ns/1000` is the delivery hop, measured, in one clock.

stdin  : line 1, the plan:
         {"pace_ms":90,"hold_ms":12,"settle_ms":1500,"chunk":25,"text":"...",
          "keys":[{"code":30,"shift":0},...]}
         (`text` is a convenience: it is converted with the US layout table below.)
         then one line per chunk: "go" types the next `chunk` keys, "end" stops.
stdout : line 1 {"ready":true,...}; one {"chunk":i,"events":[...]} per "go"; then the summary
         {"ok":true,"n":300,"clock":"CLOCK_MONOTONIC","events":[...]}.

WHY IT IS CHUNKED. Real keys go wherever the compositor thinks focus is, and this machine has
terminals on it. The caller re-verifies, between every chunk, that the page still reports keyboard
focus; if it does not it sends "end" and nothing more is typed. A single write of 300 keys could
not be stopped half way.
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
KEY_ENTER, KEY_BACKSPACE, KEY_LEFTSHIFT = 28, 14, 42

def code_for(ch):
    if ch == '\n': return (KEY_ENTER, 0)
    if ch == '\b': return (KEY_BACKSPACE, 0)
    if ch in LETTERS: return (LETTERS[ch], 0)
    if ch.lower() in LETTERS and ch.isupper(): return (LETTERS[ch.lower()], 1)
    if ch in ROW: return (ROW[ch], 0)
    if ch in SHIFTED: return (ROW[SHIFTED[ch]], 1)
    return None

def main():
    plan = json.loads(sys.stdin.readline())   # ONE line: stdin stays open for the chunk commands
    keys = plan.get('keys')
    if keys is None:
        keys = []
        for ch in plan.get('text', ''):
            c = code_for(ch)
            if c is None: json.dump({'ok': False, 'error': 'no key for %r' % ch}, sys.stdout); return 2
            keys.append({'code': c[0], 'shift': c[1]})
    pace = plan.get('pace_ms', 90) / 1000.0
    hold = plan.get('hold_ms', 12) / 1000.0
    settle = plan.get('settle_ms', 1500) / 1000.0
    chunk = int(plan.get('chunk', 0)) or len(keys)

    fd = os.open(UINPUT, os.O_WRONLY | os.O_NONBLOCK)
    for bit in (EV_KEY, EV_SYN): fcntl.ioctl(fd, UI_SET_EVBIT, bit)
    wanted = sorted({k['code'] for k in keys} | {KEY_LEFTSHIFT})
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

    out = []
    print(json.dumps({'ready': True, 'keys': len(keys), 'chunk': chunk}), flush=True)
    try:
        i = 0
        while i < len(keys):
            cmd = sys.stdin.readline().strip()
            if cmd != 'go':                       # "end", EOF, or anything unexpected: stop typing
                break
            first = i
            for _ in range(min(chunk, len(keys) - i)):
                k = keys[i]
                pkt = b''
                if k['shift']: pkt += ev(EV_KEY, KEY_LEFTSHIFT, 1)
                pkt += ev(EV_KEY, k['code'], 1) + ev(EV_SYN, SYN_REPORT, 0)
                t = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
                os.write(fd, pkt)
                out.append({'i': i, 'code': k['code'], 'shift': k['shift'], 't_ns': t})
                time.sleep(hold)
                up = ev(EV_KEY, k['code'], 0)
                if k['shift']: up += ev(EV_KEY, KEY_LEFTSHIFT, 0)
                os.write(fd, up + ev(EV_SYN, SYN_REPORT, 0))
                rest = pace - hold
                if rest > 0: time.sleep(rest)
                i += 1
            print(json.dumps({'chunk': first, 'typed': i - first}), flush=True)
    finally:
        time.sleep(0.2)
        fcntl.ioctl(fd, UI_DEV_DESTROY)
        os.close(fd)
    json.dump({'ok': True, 'n': len(out), 'requested': len(keys), 'clock': 'CLOCK_MONOTONIC',
               'device': 'quill-latency-bench (uinput, bus USB)', 'events': out}, sys.stdout)
    sys.stdout.flush()
    return 0

if __name__ == '__main__':
    sys.exit(main())
