# Screenshotting and driving a native window on Hyprland — the harness for the Gate

Research for [#7](https://github.com/danielbaldwin47/Quill/issues/7). Everything below was run on
this machine on 2026‑08‑27 unless it says otherwise; commands that were not run say so.

## The question

The Gate rebuilds `tools/shoot.mjs` (deterministic screenshots at 1440×900, dpr 2, given theme,
font, size, focus, caret position) and `tools/latency.mjs` (keystroke‑to‑paint and cold start)
against a native GTK4 window instead of headless Chromium. Which tools do the job on
Hyprland/Wayland, how do you get pixel‑identical output, and how do you measure keystroke‑to‑paint
with no `requestAnimationFrame` and no `EventTiming`?

## Answer in one line

`grim -T <toplevel-id>` for the screenshot, and the app's own `GdkFrameClock` for the latency:
GDK already receives the compositor's `wp_presentation` feedback and hands it back as
`GdkFrameTimings::presentation_time` in `CLOCK_MONOTONIC` microseconds — the same clock
`tools/uinput-keys.py` stamps each `write(2)` with, so the two subtract with no correlation trick.

## 1. Tooling inventory (this machine)

| | version | verdict |
|---|---|---|
| `grim` | 1.5.0‑2 | **the screenshot tool.** Has `-T` (foreign‑toplevel capture) |
| `slurp` | 1.5.0‑2 | interactive region picker; not needed, geometry comes from `hyprctl` |
| `wtype` | 0.4‑2 | usable for *state* setup, **not** for latency (see §4.2) |
| `ydotool` | 1.0.4‑2 | needs a `ydotoold` daemon and adds nothing over `tools/uinput-keys.py` |
| `wl-clipboard` | 1:2.3.0‑1 | for the paste regime, as today |
| Hyprland | **0.56.2** | Lua config/dispatchers — see §2, most of the old CLI is gone |
| GTK | 4.22.4 | `gtk-font-rendering` (4.16+) and `GdkFrameTimings` both present |
| python‑gobject | 3.56.3 | used for the throwaway probe; the real app is Rust + gtk4‑rs, same API |
| ImageMagick | 7.1.2.30 | `magick compare -metric AE` for the pixel diffs below |
| `wf-recorder`, `wayland-utils`, `gtk4-demos` | not installed | not needed |

Compositor protocols (from `strings /usr/bin/Hyprland`): `ext_foreign_toplevel_list_v1`,
`ext_foreign_toplevel_image_capture_source_manager_v1`, `ext_image_copy_capture_*`,
`zwlr_screencopy_*`, `wp_presentation`, `wp_fractional_scale_v1`,
`zwp_virtual_keyboard_manager_v1`. Everything the harness needs is implemented.

**The display is back.** `progress/latency-report.md` §1 records that every DRM connector read
`disconnected` and Hyprland was on a `FALLBACK` headless output. That is no longer true:

```
DP-3  Dell U2720Q  3840x2160@59.997  scale 1.5  →  2560x1440 logical
```

Note the scale: **1.5, not 2**. A 1440×900 logical window on DP‑3 is 2160×1350 device pixels, not
the 2880×1800 the reference shots are compared at. The panel cannot serve the screenshot Piece
as configured; §3 uses a dedicated output instead.

## 2. Hyprland 0.56.2 is Lua, and most of the CLI in BRIEF.md is dead

Measured, not read:

```console
$ hyprctl dispatch workspace 5
error: [string "return hl.dispatch(workspace 5)"]:1: ')' expected near '5'
 → Note: dispatch in lua is a shorthand for hl.dispatch(...), your syntax might need to be updated.

$ hyprctl keyword animations:enabled 0
keyword can't work with non-legacy parsers. Use eval.
```

The working forms:

| want | 0.56.2 |
|---|---|
| switch workspace | `hyprctl repl 'return hl.dispatch(hl.dsp.focus{workspace=5})'` |
| focus a window | `hl.dispatch(hl.dsp.focus{window=w})` where `w` comes from `hl.get_windows()` — a *class string* is silently ignored |
| move the pointer | `hl.dispatch(hl.dsp.cursor.move(x, y))` |
| launch onto a workspace | `hl.dispatch(hl.dsp.exec_cmd("[workspace 5 silent] <cmd>"))` |
| set a config value | `hyprctl eval 'hl.config({animations={enabled=false}})'` |
| configure an output | `hyprctl eval 'hl.monitor({output="HEADLESS-25", mode="3200x2000@60", position="auto", scale=2})'` |
| add a window rule | `hyprctl eval 'hl.window_rule({match={class="..."}, ...})'` |

`hyprctl eval` takes **one line**; for several statements write a file and
`hyprctl eval 'dofile("/path/rules.lua") return "ok"'`.

`bin/quill` exits 6 with a long comment claiming *"Hyprland 0.56.2's Lua dispatcher has no
workspace-switch binding this script could find"*. It is wrong — `goto_ws()` already emits the
right call, and it works in both directions:

```console
$ hyprctl repl 'return hl.dispatch(hl.dsp.focus{workspace=5})'; hyprctl activeworkspace -j | jq .id
5
$ hyprctl repl 'return hl.dispatch(hl.dsp.focus{workspace=1})'; hyprctl activeworkspace -j | jq .id
1
```

That unblocks `bin/quill --panel`, which is the only mode that can produce a real scan‑out number.

### Window‑rule keys are validated — and half the wiki names are wrong here

`hl.window_rule` rejects unknown fields, so the set is discoverable. Tested:

* **valid**: `float`, `size`, `move`, `workspace`, `no_anim`, `border_size`, `rounding`,
  `no_shadow`, `no_blur`, `no_dim`, `dim_around`, `opacity`, `tag`, `immediate`, `center`,
  `no_focus`, `no_initial_focus`, `xray`, `suppress_event`
* **rejected**: `no_border`, `no_rounding`, `shadow`, `blur` — use `border_size=0`, `rounding=0`,
  `no_shadow=true`, `no_blur=true`

`animations:enabled 0` is not needed and should not be used: it is global and would change the
user's desktop. `no_anim=true` in a class‑scoped window rule is the scoped equivalent, and
`hl.layer_rule({match={namespace=...}, no_anim=true, animation="none"})` is how Omarchy already
does it for its own layers.

**Omarchy dims every window.** `/usr/share/omarchy/default/hypr/windows.lua` tags all windows
`default-opacity` and then applies `opacity = "0.985 0.96"`. A screenshot taken through the
compositor is therefore composited at 98.5 % opacity unless the rule set cancels it with
`tag="-default-opacity"` and `opacity="1.0 1.0"`. (§3's recipe sidesteps this entirely.)

Rules added at runtime live as long as the compositor. `HL.WindowRule` exposes only
`set_enabled`/`is_enabled`, so either keep the handles in a Lua global for teardown or finish with
`hyprctl reload`.

## 3. Screenshot recipe

### 3.1 A dedicated output, so the panel is never touched and the scale is ours

DP‑3 is scale 1.5 and belongs to the user. Make our own:

```bash
before=$(hyprctl monitors -j | jq -r '[.[].name]|join(" ")')
hyprctl output create headless                       # → HEADLESS-N, 1920x1080@60, scale 2
out=$(hyprctl monitors -j | jq -r --arg b "$before" '.[]|select(($b|split(" "))|index(.name)|not)|.name')
hyprctl eval "return hl.monitor({output=\"$out\", mode=\"3200x2000@60\", position=\"auto\", scale=2})"
ws=$(hyprctl monitors -j | jq -r --arg o "$out" '.[]|select(.name==$o).activeWorkspace.id')
```

3200×2000 at scale 2 is 1600×1000 logical — room for a 1440×900 window plus margin. The output
is real: Hyprland composites and presents it at 60 Hz, and nothing appears on DP‑3. Remove it
with `hyprctl output remove "$out"` and the workspace folds back onto the panel.

### 3.2 Pin the window

```lua
-- rules.lua, loaded with: hyprctl eval 'dofile("rules.lua") return "ok"'
local C = "^dev\\.quill\\.probe$"           -- the app_id, as a regex
hl.window_rule({name="q-ws",    match={class=C}, workspace="5 silent"})
hl.window_rule({name="q-float", match={class=C}, float=true})
hl.window_rule({name="q-size",  match={class=C}, size="1440 900"})
hl.window_rule({name="q-move",  match={class=C}, move="80 50"})
hl.window_rule({name="q-deco",  match={class=C}, no_anim=true, border_size=0, rounding=0,
                                no_shadow=true, no_blur=true, no_dim=true,
                                opacity="1.0 1.0", tag="-default-opacity"})
```

`class` is the xdg‑toplevel `app_id`; for GTK4 that is the `GtkApplication` application‑id
(`dev.quill.probe` here), and it is what `hyprctl clients -j` reports as `class`.

`hyprctl clients -j` `at` and `size` are **logical (layout) coordinates**, not device pixels —
confirmed: the window reads `at [2640, 50] size [1440, 900]` while its buffer is 2880×1800.

### 3.3 Capture — `grim -T`, not `grim -g`

`grim -g "<x>,<y> <w>x<h>"` takes *layout* coordinates (so it pairs directly with `hyprctl
clients`), `-s` sets the image scale factor (default: the greatest output scale on the machine),
and `-o` and `-g` are **mutually exclusive**. It works, and it is byte‑identical run to run:

```console
$ grim -g "2640,50 1440x900" -s 2 a.png   # 2880x1800, sha 53cdb452…
$ grim -g "2640,50 1440x900" -s 2 b.png   # identical bytes
```

**But a region capture is a capture of the compositor's output, layers included.** On this
machine `hyprctl layers` shows `omarchy-bar` at level 2 (top 26 logical px) and
`omarchy-notifications` at level 3 covering the whole output — and the very first test capture came
back with a *"Process crashed: python3.14"* toast painted over the top‑right of the document. That
is an unfixable source of flake in a judging harness.

`grim -T` captures the toplevel's own buffer through
`ext_foreign_toplevel_image_capture_source_manager_v1`. No layers, no bar, no notifications, no
cursor, no compositor border/shadow/rounding, and no compositor opacity rule:

```console
$ grim -T 18000403 t1.png
$ identify t1.png                       # 2880x1800, ~100 ms, sha 8b2c1421…
```

The identifier is opaque and **is not in `hyprctl clients`**. It comes from
`ext_foreign_toplevel_handle_v1.identifier`; recover it by making grim enumerate the list and
reading the trace:

```bash
toplevel_id() {   # $1 = app_id
  WAYLAND_DEBUG=1 grim -T __none__ /dev/null 2>&1 \
  | sed 's/\x1b\[[0-9;]*m//g' \
  | awk -v app="$1" '
      /\.identifier\(/ { match($0, /identifier\("([^"]*)"\)/, m); id = m[1] }
      $0 ~ "\\.app_id\\(\"" app "\"\\)" { print id; exit }'
}
grim -T "$(toplevel_id dev.quill.probe)" shots/type/x.png
```

Caveat: `-T` captures the client's buffer, so **client‑side decorations are in the shot**. Keep
`set_decorated(false)` (or CSD off) for judged screenshots, as the probe does.

### 3.4 It is genuinely deterministic

Three *independent process launches*, screenshot each, kill each:

```
r1.png r2.png r3.png   2880x1800   sha256 0bb34211b53bf695…   identical
magick compare -metric AE r1.png r2.png  →  0 (0)
```

### 3.5 The four knobs that break it

| knob | measured effect | fix |
|---|---|---|
| **caret blink** | two shots of a *focused* window 620 ms apart differ by **76 px** | `gtk-cursor-blink = false` (and `gtk-enable-animations = false`) under a `--deterministic` flag. Verified: 3 shots, 0 differing pixels |
| **font hinting** | `hintslight` vs `hintfull` = **1 728 px**; vs `hintnone` = **6 602 px** | pin `gtk-xft-antialias/hinting/hintstyle/rgba/dpi`, `gtk-hint-font-metrics`, and set `gtk-font-rendering = MANUAL` (GTK 4.16+) so GTK honours them instead of choosing |
| **GSK renderer** | `gl` = `ngl` = `vulkan`, byte‑identical; **`cairo` differs by 8 633 px** | export `GSK_RENDERER=gl` |
| **`GDK_SCALE`** | Omarchy exports `GDK_SCALE=2` globally (`hl.env` in `~/.config/hypr/monitors.lua`). Harmless on a scale‑2 output, but on DP‑3 (scale 1.5) it forces a 2× buffer the compositor then rescales | `unset GDK_SCALE`; let `wp_fractional_scale_v1` and the pinned output decide |

This machine's current GTK defaults, for the record (`gtk4-query-settings`): `gtk-xft-antialias 1`,
`gtk-xft-hinting 1`, `gtk-xft-hintstyle "hintslight"`, `gtk-xft-rgba "none"`, `gtk-xft-dpi 98304`
(= 96 dpi), `gtk-hint-font-metrics TRUE`, `gtk-font-rendering AUTOMATIC`, `gtk-cursor-blink TRUE`,
`gtk-enable-animations TRUE`. Three of those are wrong for judging.

`shoot.mjs`'s other flags (`--theme`, `--font`, `--size`, `--focus`, `--typewriter`, `--chrome`,
`--text`, `--caret`, `--select`, `--scroll`, `--nocaret`) have no compositor analogue and must
become **command‑line flags on the native app itself**, applied before the first frame. That is the
one structural change from the Playwright harness: the state used to be injected from outside
through CDP; now the app has to be able to open in a given state.

## 4. Latency recipe

### 4.1 The measurement: `GdkFrameClock`, which is `wp_presentation` in disguise

`wp_presentation`'s `presented` event carries the time the update *"turned into light the first
time on the surface's main output"*, in the clock the compositor announces via `clock_id`
(`/usr/share/wayland-protocols/stable/presentation-time/presentation-time.xml`). GDK's Wayland
backend consumes it (`gdk_wayland_presentation_feedback_presented` in `libgtk-4.so.1`) and exposes
it as `GdkFrameTimings::presentation_time`.

Measured: `presentation_time` is in the **same domain as `clock_gettime(CLOCK_MONOTONIC)`**, in
microseconds. From one record —

```json
{"t_handler_ns":169962975970041, "present_us":169962979465, "refresh_us":16666,
 "lat_handler_ms":3.494959}
```

169 962 975 970 041 ns / 1000 = 169 962 975 970 µs, and 169 962 979 465 − that = 3 495 µs. Same
clock, no offset. So it subtracts directly against the `CLOCK_MONOTONIC` nanosecond stamp
`tools/uinput-keys.py` already takes immediately before each `write(2)` to `/dev/uinput`.

The instrumentation, in the app, behind `--measure` (Python here; gtk4‑rs is the same API):

```python
key = Gtk.EventControllerKey()
key.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)   # or GtkTextView eats it first
key.connect('key-pressed', on_key)                         # t0: CLOCK_MONOTONIC + GdkEvent time
window.add_controller(key)

fc = window.get_frame_clock()
fc.connect('after-paint', after_paint)   # the frame counter that carried this edit

# later, once the frame's feedback has landed:
ti = fc.get_timings(n)
if ti.get_complete():
    ti.get_presentation_time()           # µs, CLOCK_MONOTONIC — the headline
    ti.get_frame_time()                  # µs, when the frame began
    ti.get_predicted_presentation_time()
    ti.get_refresh_interval()            # 16666 on a 60 Hz output
```

Three traps, all hit:

1. A bubble‑phase key controller on the window **never fires** — `GtkTextView` consumes the key.
   Use `PropagationPhase.CAPTURE`.
2. `GdkFrameTimings` only becomes `complete` once a *later* frame runs. An idle window's last
   frame never completes, and the frame at map time can end with `presentation_time == 0`. Drain
   on a timer and, for cold start, scan forward for the first frame that is both complete and has
   a non‑zero presentation time.
3. Don't truncate the JSONL from the shell while the app holds it open — the offset is preserved
   and you get a NUL‑padded file.

### 4.2 The input: `/dev/uinput`, not `wtype`

`wtype` drives `zwp_virtual_keyboard_v1`, and **the events arrive with no timestamp**:
`gdk_event_get_time()` returned `0` for every `wtype` key, and a real libinput value
(`t_evdev_us = 169983352000`) for every `/dev/uinput` key. A virtual keyboard cannot give you a
hardware t0, so `wtype` is fine for setting up state and useless for the bench.
`tools/uinput-keys.py` stays exactly as it is; `ydotool` would need a `ydotoold` daemon and adds
nothing.

`gdk_event_get_time()` is milliseconds, so it quantises at 1 ms. Use it as a cross‑check and take
the real t0 from `uinput-keys.py`'s own `t_ns`. Pair the two streams by **`gdk_keycode = evdev
code + 8`** (verified: `'u'` = evdev 22 → GDK 30) and align greedily rather than by index — a stray
`Super` press from the user showed up mid‑run in one session.

### 4.3 Measured, end to end

160 keys at 90 ms pace into the probe (1 200‑word document, `GSK_RENDERER=gl`, headless output at
60 Hz scale 2). The run stopped itself after 40 keys when focus was lost, which is the chunk
protocol working:

| milestone | n | mean | sd | p50 | p95 | max |
|---|---|---|---|---|---|---|
| `write(2)` on `/dev/uinput` → **presented** | 36 | **2.13** | 0.89 | 1.92 | 4.69 | 6.01 |
| `GdkEvent` libinput stamp → presented | 36 | 2.59 | 0.90 | 2.46 | 4.78 | 6.47 |
| first GTK handler → presented | 36 | 1.65 | 0.74 | 1.47 | 3.12 | 5.33 |

Cold start, `exec` → the first frame carrying real presentation feedback, five runs:
**271.5, 197.9, 189.7, 191.9, 196.4 ms** (a PyGObject app; Rust will be far below this). The
caller stamps `CLOCK_MONOTONIC` immediately before the exec and passes it in the environment;
the app prints the delta itself. No CDP, no `--remote-debugging-port`.

**Read those numbers with §6's caveat.** Every one is under a 16.67 ms refresh interval, which
means the headless output is acknowledging presentation almost immediately rather than at a
scan‑out cadence — the same 1.98 ms commit→present the round‑4 report measured on its virtual
output. It is an honest *application* number and it is not keyboard‑to‑photon.

### 4.4 Keeping the keys in our own window

Both injection paths go to whatever the compositor thinks has keyboard focus, and this went wrong
in testing: focus silently reverted to the user's browser (Hyprland's `follow_mouse`, because the
pointer was still on DP‑3) and a `wtype` burst landed in their YouTube tab. Three defences, all
needed:

1. Move the pointer onto the target output *before* focusing:
   `hl.dispatch(hl.dsp.cursor.move(x, y))` then `hl.dispatch(hl.dsp.focus{window=w})`.
2. Re‑read `hyprctl activewindow -j` and retry; give up rather than type blind.
3. Keep `tools/uinput-keys.py`'s chunk protocol and re‑verify focus between every chunk. It
   already earns its keep.

`tools/idle-check.py` still works unchanged and is still the right gate for anything that takes
the panel — it returned 0 on an idle machine and then correctly refused with
`idle-check: activity on Logitech MX Keys` when the user touched the keyboard.

## 5. Determinism checklist

Before a judged screenshot:

- [ ] `unset GDK_SCALE`; `GSK_RENDERER=gl`; `GDK_BACKEND=wayland`; `GTK_A11Y=none`
- [ ] dedicated headless output at the exact mode and **integer** scale the shot needs
      (2880×1800 @ 2 for a 1440×900 dpr‑2 shot, plus margin); never DP‑3, which is scale 1.5
- [ ] window rules on the app‑id: `float`, `size`, `move`, `workspace N silent`, `no_anim`,
      `border_size=0`, `rounding=0`, `no_shadow`, `no_blur`, `no_dim`,
      `opacity="1.0 1.0"`, `tag="-default-opacity"`
- [ ] app‑side, under one `--deterministic` flag: `gtk-enable-animations=false`,
      `gtk-cursor-blink=false`, `gtk-font-rendering=MANUAL`, `gtk-xft-antialias=1`,
      `gtk-xft-hinting=1`, `gtk-xft-hintstyle=hintslight`, `gtk-xft-rgba=none`,
      `gtk-xft-dpi=96*1024`, `gtk-hint-font-metrics=true`, CSD off
- [ ] fonts from the repo, not the system: `FONTCONFIG_FILE` pointing at a harness `fonts.conf`
      that lists `app/fonts/` and nothing else, so a system font update cannot move a glyph
- [ ] capture with `grim -T <toplevel-id>`, never `grim -g`, so no bar / notification / cursor /
      compositor decoration can land in the frame
- [ ] assert the PNG's pixel size before writing it; assert two consecutive captures are identical
- [ ] tear down: `hyprctl output remove <out>`, and either `set_enabled(false)` on the kept rule
      handles or `hyprctl reload`

Before a latency run, additionally:

- [ ] `tools/idle-check.py` gate for anything that takes the panel
- [ ] pointer moved onto the target output, focus verified against `hyprctl activewindow -j`,
      re‑verified between chunks
- [ ] record `hyprctl monitors -j` (mode, refresh, scale, vrr) and the app build fingerprint in
      the results file, as `latency.mjs` does today
- [ ] every keystroke accounted for: keys sent == keys the app saw == keys with a
      `presentation_time`; alignment done by `evdev + 8`, not by index

## 6. Open risks

1. **A headless output is not a display.** Its `presented` timestamps are the compositor
   acknowledging a commit, not a scan‑out; that is why §4.3's numbers sit below one refresh
   interval. The keyboard‑to‑photon bar in `progress/latency-report.md` (Hume: Sublime
   32.5 ± 4.0 ms on 60 Hz) can only be answered on DP‑3. That is now possible — the panel is
   connected again and §2 fixes the workspace switch `bin/quill --panel` was giving up on — but it
   was not run here: `idle-check.py` refused, correctly, because the user was at the keyboard.
2. **`grim -T` identifier discovery is a hack.** Parsing `WAYLAND_DEBUG=1` output is stable enough
   in practice but is not an interface. If it ever breaks, the fallback is a ~100‑line
   `ext_foreign_toplevel_list_v1` client, or `grim -g` with the notification daemon paused.
3. **CSD.** `-T` captures the client buffer, so if the native app ever grows client‑side
   decorations (GTK4's default `GtkHeaderBar` path) the shadow and rounding come with it and the
   pixel size stops matching the reference crop.
4. **Fractional scale.** Everything here holds because the capture output is forced to integer
   scale 2. On a 1.5 output GTK renders through `wp_fractional_scale_v1` and glyph positions are
   not a clean 2× of the logical layout; do not judge on DP‑3.
5. **Runtime state leaks.** Window rules and created outputs persist for the compositor's
   lifetime. A crashed harness leaves a stray HEADLESS output and a rule set behind; the teardown
   has to be trap‑based, as `bin/quill` already does for its virtual output.
6. **The probe is Python.** The API surface (`GdkFrameClock`, `GdkFrameTimings`,
   `GtkEventControllerKey`, `GtkSettings`) is identical in gtk4‑rs, but the cold‑start and
   handler‑cost numbers in §4.3 are a Python interpreter's, not Quill's, and should not be
   compared against the ≤ 5 ms bar.
7. **One machine, one compositor, one 60 Hz mode**, exactly as the round‑4 report says of itself.

## 7. Recipes, condensed

```bash
# ---- screenshot -------------------------------------------------------------
before=$(hyprctl monitors -j | jq -r '[.[].name]|join(" ")')
hyprctl output create headless
out=$(hyprctl monitors -j | jq -r --arg b "$before" '.[]|select(($b|split(" "))|index(.name)|not)|.name')
hyprctl eval "return hl.monitor({output=\"$out\",mode=\"3200x2000@60\",position=\"auto\",scale=2})"
ws=$(hyprctl monitors -j | jq -r --arg o "$out" '.[]|select(.name==$o).activeWorkspace.id')
hyprctl eval 'dofile("tools/hypr/shoot-rules.lua") return "ok"'

hyprctl repl "return hl.dispatch(hl.dsp.exec_cmd(\"[workspace $ws silent] \
  env -u GDK_SCALE GSK_RENDERER=gl GTK_A11Y=none \
  quill --deterministic --theme light --font duo --size 18 --focus off \
        --caret 'the boat' doc.md\"))"

id=$(toplevel_id dev.quill.Quill)          # see §3.3
grim -T "$id" shots/type/x.png             # 2880x1800, byte-stable
hyprctl output remove "$out"

# ---- latency ----------------------------------------------------------------
t0=$(python3 -c 'import time;print(time.clock_gettime_ns(time.CLOCK_MONOTONIC))')
hyprctl repl "return hl.dispatch(hl.dsp.exec_cmd(\"[workspace $ws silent] \
  env -u GDK_SCALE QUILL_T0_NS=$t0 GSK_RENDERER=gl \
  quill --deterministic --measure out.jsonl doc.md\"))"
# focus: cursor.move onto $out, then focus{window=w}, then verify activewindow
python3 tools/uinput-keys.py < plan.json   # chunked, focus re-checked between chunks
# join: present_us*1000 - t_ns, paired by gdk_keycode == evdev + 8
```

The full probe used for every number above is not committed; it is ~120 lines of PyGObject and its
shape is in §4.1.
