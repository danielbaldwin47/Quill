# Quill's palette, in Omarchy's template contract (docs/design.md § The palette
# is a file). Copy this file into ~/.config/omarchy/themed/ and every
# `omarchy theme set` renders it into ~/.local/state/omarchy/current/theme/quill.toml;
# `palette = "~/.local/state/omarchy/current/theme/quill.toml"` in Quill's
# settings.toml is the other half. The keys are Quill's nineteen roles; the
# values are the theme's colours, and `mix a b N%` is Omarchy's blend, N% of b
# laid over a. Only the table the theme's `mode` names is written, so a dark
# theme leaves Quill's light ground designed rather than guessed.

[{{ mode }}]
paper = "{{ background }}"
ink = "{{ foreground }}"
ink_dim = "{{ dark_foreground }}"
mark = "{{ muted }}"
accent = "{{ accent }}"
link = "{{ accent }}"
link_rule = "{{ mix accent background 50% }}"
selection = "{{ selection }}"
selection_idle = "{{ mix selection background 50% }}"
code_bg = "{{ mix background foreground 6% }}"
rule = "{{ muted }}"
shadow = "{{ darker_background }}"
chrome_fg = "{{ dark_foreground }}"
chrome_fg_strong = "{{ foreground }}"
syntax_noun = "{{ red }}"
syntax_verb = "{{ blue }}"
syntax_adjective = "{{ brown }}"
syntax_adverb = "{{ magenta }}"
syntax_conjunction = "{{ green }}"
