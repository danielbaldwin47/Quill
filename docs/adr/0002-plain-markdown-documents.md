# Documents are plain Markdown files with no proprietary metadata

Quill reads and writes CommonMark plus GFM extensions and footnotes, and stores nothing about a Document outside the file itself. A user with an existing iA Writer or Obsidian folder must be able to point Quill at it and leave without residue. Anything Quill needs to remember (window state, last file, settings) lives in XDG config, never in the Library.
