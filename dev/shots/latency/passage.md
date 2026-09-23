# Alice’s Adventures in Wonderland

by Lewis Carroll — *working draft, marked up for the copy-edit pass*

## Editor’s note

This file is the [Project Gutenberg text](https://www.gutenberg.org/ebooks/11) reflowed one paragraph per line, with the original italics kept as Markdown emphasis. Before it goes to the typesetter:

- [x] Reflow paragraphs to one logical line
- [x] Keep Carroll’s italics (`_very_`, `_took a watch out of its waistcoat-pocket_`)
- [ ] Check the verse indentation against the 1897 edition
- [ ] Decide on **spaced en dashes** vs. em dashes throughout

> “What is the use of a book,” thought Alice, “without pictures or conversations?”

Build the reading copy with:

```sh
quill export alice.md --template classic --measure 66ch > alice.html
```

## 1. Down the Rabbit-Hole

Alice was beginning to get very tired of sitting by her sister on the bank, and of hav