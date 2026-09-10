#!/usr/bin/env python3
"""DEBUG-327 round 3: what every tool here needs from one archived session.

An archived stage directory holds, per session, the capture the app wrote
(`capture-<n>.jsonl`, one line per key, the production file) and the probe file
beside it (`capture-<n>.probe.jsonl`, one line per observation, added by
`instrumentation.patch` and never read by anything that scores).
"""
import json
import sys

def load(path):
    """Every line of a JSONL file, as objects."""
    return [json.loads(line) for line in open(path) if line.strip()]

class Session:
    """One session of one archived run: its keys, and its probe observations."""

    def __init__(self, stage, session):
        self.stage = stage
        self.session = session
        self.keys = load(f'{stage}/capture-{session}.jsonl')
        self.probe = load(f'{stage}/capture-{session}.probe.jsonl')
        self.paint = {e['frame']: e['at_us'] for e in self.probe if e['kind'] == 'paint'}
        self.before = {e['frame']: e['at_us'] for e in self.probe
                       if e['kind'] == 'phase' and e['name'] == 'before'}
        self.paints = sorted(self.paint.values())

    def split(self, key):
        """One key as wait / draw / comp / total, in milliseconds.

        wait = handler to the frame clock's `before-paint` (or after-paint, when
        the run carried no phase probe), draw = `before-paint` to after-paint,
        comp = after-paint to presented. `None` for a key with no presentation
        time or no paint probe for its frame.
        """
        frame, handler, presented = key['frame'], key['handler_us'], key['present_us']
        after = self.paint.get(frame)
        if presented is None or after is None:
            return None
        before = self.before.get(frame, after)
        return ((before - handler) / 1000.0, (after - before) / 1000.0,
                (presented - after) / 1000.0, (presented - handler) / 1000.0)

def when(event):
    """The monotonic microsecond an observation belongs at, whatever its kind."""
    return event.get('at_us') or event.get('to_us') or event.get('end_us') or event.get('after_us')

def render(event, relative_to):
    """One observation as a line, its time relative to `relative_to`."""
    at = when(event)
    if at is None:
        return None
    rel = f'{(at - relative_to) / 1000.0:>+9.2f} ms  '
    kind = event['kind']
    if kind == 'phase':
        return f'{rel}phase {event["name"]:<12} frame {event["frame"]}'
    if kind == 'paint':
        return f'{rel}after-paint     frame {event["frame"]}'
    if kind == 'idle':
        return f'{rel}idle            frame {event["frame"]}'
    if kind == 'tail':
        return f'{rel}TAIL queue_draw frame {event["frame"]}'
    if kind == 'busy':
        return f'{rel}busy            {(event["to_us"] - event["from_us"]) / 1000.0:.2f} ms'
    if kind == 'stage':
        return f'{rel}stage {event["name"]:<12} {(event["end_us"] - event["start_us"]) / 1000.0:.2f} ms'
    if kind == 'accept':
        return f'{rel}accept {event["consumer"]:<8} frame {event["frame"]} present={event["present_us"]}'
    if kind == 'reread':
        return f'{rel}reread          frame {event["frame"]} present={event["present_us"]}'
    return f'{rel}{kind}'

def flags(argv, taking, switches=()):
    """The stage directories, and the flags named in `taking` and `switches`.

    `taking` maps a flag to the function its value is read with; `switches` are
    the flags that stand alone. An unknown flag is an error rather than silence,
    because a mistyped one would otherwise change what is measured without
    saying so.
    """
    stages, said = [], {}
    rest = list(argv)
    while rest:
        word = rest.pop(0)
        if word in taking:
            said[word] = taking[word](rest.pop(0))
        elif word in switches:
            said[word] = True
        elif word.startswith('--'):
            raise SystemExit(f'{sys.argv[0]}: unknown flag {word}')
        else:
            stages.append(word)
    return stages, said
