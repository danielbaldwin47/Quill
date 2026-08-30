#!/bin/bash
# Driver for iA Writer for Mac. One verb per invocation.
set -u
APP="iA Writer"

act()   { osascript -e "tell application \"$APP\" to activate"; sleep 0.4; }
menu()  { # menu <menubar item> <item>
  osascript -e "tell application \"System Events\" to tell process \"$APP\" to click menu item \"$2\" of menu 1 of menu bar item \"$1\" of menu bar 1"; }
submenu() { # submenu <menubar item> <parent> <item>
  osascript -e "tell application \"System Events\" to tell process \"$APP\" to click menu item \"$3\" of menu 1 of menu item \"$2\" of menu 1 of menu bar item \"$1\" of menu bar 1"; }
items() { # items <menubar item>
  osascript -e "tell application \"System Events\" to tell process \"$APP\" to return name of every menu item of menu 1 of menu bar item \"$1\" of menu bar 1"; }
key()   { # key <char> <modifiers e.g. command down,shift down>
  if [ $# -ge 2 ]; then
    osascript -e "tell application \"System Events\" to keystroke \"$1\" using {$2}"
  else
    osascript -e "tell application \"System Events\" to keystroke \"$1\"";
  fi; }
code()  { # code <keycode> [modifiers]
  if [ $# -ge 2 ]; then
    osascript -e "tell application \"System Events\" to key code $1 using {$2}"
  else
    osascript -e "tell application \"System Events\" to key code $1";
  fi; }
bounds(){ osascript -e "tell application \"$APP\" to return bounds of window 1"; }
setbounds(){ osascript -e "tell application \"$APP\" to set bounds of window 1 to {$1,$2,$3,$4}"; }
"$@"
