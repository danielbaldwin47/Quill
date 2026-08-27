# Settings live in one TOML file under XDG config, not in GSettings

Everything Quill remembers about the writer's preferences is one file, `$XDG_CONFIG_HOME/quill/settings.toml`;
everything it remembers about the last session (window geometry, last Document, caret positions,
recents) is under `$XDG_STATE_HOME/quill/`. GSettings, the GTK-native store, needs a compiled schema
installed by the package and is opaque to the owner and to agents; a TOML file can be read, diffed and
hand-edited, is trivially testable with no display, and matches the Templates, which are TOML already.
Settled in [#11](https://github.com/danielbaldwin47/Quill/issues/11).

Config holds only what the writer chose; state holds what the app observed. The split keeps config
hand-editable and lets state churn on every launch without touching it. Nothing about a Document is
stored in either ([ADR 0002](0002-plain-markdown-documents.md)).
