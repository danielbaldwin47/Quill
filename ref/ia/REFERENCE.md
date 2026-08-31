# iA Writer — Reference Material for Pixel-Level Comparison

Collected 2026-08-26. Everything below is either (a) quoted verbatim from iA's own pages / repos, (b) MEASURED from a downloaded screenshot (method stated), or (c) marked UNVERIFIED. Nothing is invented.

Directory layout:

```
ref/ia/
  REFERENCE.md          this file
  shots/                49 real app screenshots (App Store, Microsoft Store, ia.net)
  shots/mac-native/     76 captures of iA Writer for Mac running (§1.1)
  mac-native/{NOTES,VERDICTS}.md   how those captures were taken, and every claim judged against them
  mac-native/rig/       the driver that took them
  fonts/{Duo,Quattro,Mono}/   static TTF + WOFF2 + variable TTF, OFL license, readmes
  templates/            clone of github.com/iainc/iA-Writer-Templates (Example + GitHub templates)
  sources/              plain-text extracts of every ia.net page quoted here
```

---

## 1. Screenshots (`shots/`)

Dimensions measured with ImageMagick `identify`. All App Store / Microsoft Store images are marketing frames that contain an unscaled-looking crop of the real editor UI (real font rendering, real caret, real chrome where shown). Images from ia.net are either raw window captures or crops of them.

Recurring passages (transcribed once, referenced by name below):

* **ALICE-FALL** (light and dark variants; App Store, MS Store, iPhone, iPad):
  `Down, down, down. Would the fall *never* come to an end? “I must actually be getting somewhere near the so-called centre of the earth` — caret after `earth`, no closing quote. In the Focus-Mode variant the first sentence (`Down, down, down. Would the fall *never* come to an end?`) is dimmed. In the Style-Check variant `actually` and `so-called` are struck through and `so-called` is selected. In the wikilink variant the word `actually` is absent and the text ends `…centre of the [[earth]]`.
* **AUTHORSHIP** (dark; App Store 02, MS Store 02, iPhone 02, iPad 02):
  `*Authorship* separates the text you typed and the text you pasted from AI. By dimming the text you pasted from other sources, you make sure that your voice is what comes through on the page.` (blank line) `It highlights the text you craft yourself, and it keeps track of your references. Ensuring that your own personality stands out in the final product.` — caret after `yourself`; second paragraph is multi-coloured per author.
* **RABBIT-HOLE-SUMMARY** (ia.net support images):
  `# 1. Down the Rabbit Hole` / `## Into the rabbit hole` / (blank) / `Alice is feeling bored and *drowsy* while sitting on the riverbank with her elder sister who is reading a book with no pictures or conversations. She then notices a talking, clothed White Rabbit with a pocket watch run past. She follows it down a rabbit hole when suddenly she falls a long way to a curious hall with many locked doors of all sizes. She finds a small key to a door too small for her to fit through, but through it she sees an attractive garden. She then discovers a bottle on a table labelled “Drink me,” the contents of which cause her to shrink too small to reach the key which she has left on the table. She eats a cake with “Eat me" written on it in currants.` / (blank) / `Alice was *beginning* *to* get very tired of sitting by her sister on the bank, and of having nothing to do: once or twice she had peeped`
* **WRITING-WELL** (ia.net landing, light, Focus Mode paragraph):
  `Writing well is not a matter of the right app, the perfect font or a certain tool. It requires focus-good writing needs your full, undivided attention. If you want to write well-in fact, if you want to do anything well-you need to concentrate.` (dimmed) / (blank) / `There are many ways to succeed at writing, and there are even more ways to fail. However, successful writing is never a matter of luck. To write well, you need to be fully awake, dead serious and unconditionally enjoy what you do-like a child, when it plays. ` (bright, caret at end after a space).
* **TRIM** (syntax highlight): `Trim and tighten: You can employ Syntax Highlight, which helps you spot weak verbs, unwanted repetitions, and eliminate clutter. Together with Style Check, it'll make you a more persuasive writer.` Colouring: blue = Trim, tighten, can, employ, helps, spot, eliminate, 'll, make; green = and (×2); brown = weak, unwanted, persuasive; purple = Together, more.
* **RABBIT-HOLE-CH1** (App Store 09 / MS Store 07, light): `# Into the Rabbit Hole` / `In another moment down went Alice after it, never once considering how in the world she was to get out again.` / (blank) / `alice.jpg` (content-block chip, caret after it) / (blank) / `The rabbit-hole went straight on like a tunnel for some way, and then dipped suddenly down, so suddenly that Alice had not a moment to think about stopping herself before she found herself falling down a very deep well.`

| File | Source URL | px | What it shows | Text |
|---|---|---|---|---|
| appstore-mac-01-light-editor-hero.png | https://apps.apple.com/app/ia-writer/id775737590 (Mac_01.png via iTunes lookup API) | 2560×1600 | Mac, light, Duo, no chrome, blue caret; frame "The Original Focused Writing App / Apple Design Award Finalist" | ALICE-FALL |
| appstore-mac-02-dark-authorship.png | same listing, Mac_02.png | 2560×1600 | Mac, dark, Duo, title bar with `Alice in Wonderland.md`, focus-dropdown, search and preview (▶) buttons; Authorship colours | AUTHORSHIP |
| appstore-mac-03-dark-syntax-highlight.png | Mac_03.png | 2560×1600 | Mac, dark, Duo, Syntax Highlight on, no chrome | ALICE-FALL (coloured) |
| appstore-mac-04-dark-style-check-selection.png | Mac_04.png | 2560×1600 | Mac, dark, Style Check strike-throughs + text selection with iOS-style blue handles, title bar | ALICE-FALL (style-check) |
| appstore-mac-05-light-library.png | Mac_05.png | 2560×1600 | Mac, light, Library (file list with excerpts, folders, blue selection bar) beside editor showing content-block chips | Library: `95% Web Design is Typography.txt 30.09.25 / 95% of the information on the web is written language. It is only logical to say that a web`; folder `Alice in Wonderland`; `0 Alice In Wonderland - main.txt 22.08.24 / 1.1 Down the rabbit hole.txt 1.2 The white rabbit.txt 1.3 Down down down.txt 2.1 Pool of`; folder `1 - Down the Rabbit Hole`; `1.1 Down the rabbit hole.txt 18.08.25 / Down the Rabbit-Hole Alice, growing increasingly weary of sitting beside her`; `1.2 The white rabbit.txt 21.08.24 / Alice was not a bit hurt, and she jumped up on to her feet in a moment: she looked up,`; `1.3 Down down down.txt 21.08.24 / And so it was indeed: she was now only ten inches high, and her face brightened up at`. Editor chips: `1.1 Down the rabbit hole.txt`, `1.2 The white rabbit.txt`, `1.3 Down down down.txt`, `2.1 Pool of tears.txt`, `2.2 Little mouse.txt`, `3.1 Caucus-Race.txt` |
| appstore-mac-06-light-focus-sentence.png | Mac_06.png | 2560×1600 | Mac, light, Focus Mode: Sentence, title bar visible | ALICE-FALL (first sentence dimmed) |
| appstore-mac-07-dark-wikilink-autocomplete.png | Mac_07.png | 2560×1600 | Mac, dark, `[[earth]]` wikilink with autocomplete popup | ALICE-FALL (wikilink). Popup: `Earth.txt / iCloud › Alice` (selected, ↩), `Research Chemistry.txt / iCloud › Homework › Earth`, `Chapter 3.txt / iCloud › Earth` |
| appstore-mac-08-dark-markdown-heading.png | Mac_08.png | 2560×1600 | Mac, dark, title bar; heading line `# Into the Rabbit Hole` rendered extremely faint (marketing fade — UNVERIFIED whether this is an app state) | `# Into the Rabbit Hole` + ALICE-FALL |
| appstore-mac-09-light-content-block.png | Mac_09.png | 2560×1600 | Mac, light, H1 with `#` hanging in margin, content-block chip `alice.jpg` with caret | RABBIT-HOLE-CH1 |
| appstore-mac-10-light-preview.png | Mac_10.png | 2560×1600 | Mac, light, Preview (serif template), full toolbar (sidebar toggle, ‹ ›, file name, focus dropdown, search, blue ▶) | `Into the Rabbit Hole` / `In another moment down went Alice after it, never once considering how in the world she was to get out again.` + Tenniel flamingo illustration |
| appstore-iphone-01..04-*.png | https://apps.apple.com/app/ia-writer/id775737172 (iPhone 6.9" 01–04) | 2796×1290 each | iPhone frames of the same four first slides (light hero, dark authorship, dark syntax, dark style-check) | ALICE-FALL / AUTHORSHIP |
| appstore-ipad-01-light-editor-hero.png, appstore-ipad-02-dark-authorship.png | same listing (iPad Pro 12.9" 01, 02) | 2732×2048 | iPad frames | ALICE-FALL / AUTHORSHIP |
| msstore-win-01-light-editor-hero.png | https://apps.microsoft.com/detail/XPFM5GFVD6WS74 (via storeedgefd API) | 1440×900 | Windows, light, no chrome, "SWISS MADE" badge | ALICE-FALL |
| msstore-win-02-dark-authorship.png | same | 1440×900 | Windows, dark; Win title bar `Authorship - iA Writer`, menu `Format Focus Authors View Help`, ▷ preview button | AUTHORSHIP |
| msstore-win-03-dark-syntax-highlight.png | same | 1440×900 | Windows, dark, Syntax Highlight | ALICE-FALL (coloured) |
| msstore-win-04-dark-style-check-selection.png | same | 1440×900 | Windows, dark, Style Check + selection, title `Alice in Wonderland - iA Writer` | ALICE-FALL (style-check) |
| msstore-win-05-light-library-dynamic-outline.png | same | 1440×900 | Windows, light, Library + Dynamic Outline + editor with `####` heading | Library: `LIBRARY`, `Documents ›`, `Alice ›`; list `Alice / Sort By Date`: `8. The Queen's Croquet-Gr... 17/02/2018 *Alice leaves the tea party and enters the garden where she comes upon thr...`; `7. A Mad Tea-Party.txt 17/02/2018` outline `7. A Mad Tea-Party / At the Large Table / Have Some Wine / Answer / Break the Silence / Puzzled / The Speech / "Not I!" / Making it Stop / "Tell us a Story" / Tea, Bread and Butter / From Memory`; `6. Pig and Pepper.txt 17/02/2018 *A Fish-Footman has an invitation for the Duchess of the house, which he del...`; `5. Advice from a Caterpillar... 17/02/2018`. Editor (`7. A Mad Tea-Party.txt - iA Writer`): `— "It *is* the same thing with you," said the Hatt… the conversation dropped, and the party sat silent … minute, while Alice thought over all she could rem… ravens and writing-desks, which wasn't much.` / `#### Break the Silence` / `The Hatter was the first to break the silence. "Wha… month is it?" he said, turning to Alice: he had tak… out of his pocket, and was looking at it uneasily, … every now and then, and holding it to his ear.` / `Alice considered a little, and then said "The fourth…` / `— "Two days wrong!" sighed the Hatter. "I told you…` (right edge cropped) |
| msstore-win-06-light-focus-sentence.png | same | 1440×900 | Windows, light, Focus Sentence, Win title bar | ALICE-FALL (dimmed first sentence) |
| msstore-win-07-light-content-block.png | same | 1440×900 | Windows, light, `#` hanging, chip `alice.jpg` | RABBIT-HOLE-CH1 |
| msstore-win-08-light-preview-split.png | same | 1440×900 | Windows preview; toolbar dropdowns `Modern (Sans)` and `Split, Web` | `Into the Rabbit Hole` / `In another moment down went Alice after it, never once considering how in the world she was to get out again.` |
| ianet-mac-light-focus-paragraph-alice.webp | https://static.ia.net/writer/iaw-alice-focus-desktop-duzlok.webp (ia.net/writer) | 3234×2358 | Mac, light (#f7f7f7), Duo, Focus Mode Paragraph, no chrome, rounded window corners, caret at end. **Primary metrics source** | WRITING-WELL |
| ianet-mac-light-focus-paragraph-alice-slide.png | https://static.ia.net/writer/landing/iA-Writer-in-Focus-Mode-slide.png | 2446×1781 | Same as above, different crop | WRITING-WELL |
| ianet-mac-dark-editor-with-preview.png | https://static.ia.net/writer/landing/iAW-alice-writing-formatting-desktop.png | 3150×1680 | Mac, dark editor (left) + light preview (right), headings with hanging `#`/`##` | RABBIT-HOLE-SUMMARY first paragraph only, caret at end; preview `1. Down the Rabbit Hole` (h1) / `Into the rabbit hole` (h2) + paragraph |
| ianet-mac-light-syntax-highlight.png | https://static.ia.net/writer/landing/iAW-syntax-highlight-desktop.png | 2512×1676 | Mac, light (#f7f7f7), Syntax Highlight, lossless palette PNG (exact colours) | TRIM |
| ianet-ios-light-syntax-highlight.png | https://static.ia.net/writer/landing/iAW-syntax-highlight-mobile.png | 1419×1478 | iPhone frame, light, Syntax Highlight | TRIM |
| ianet-mac-dark-focus-sentence-stylecheck.webp | https://static.ia.net/2023/06/focus.webp (support pages) | 2048×1152 | Mac, dark, Focus Sentence + Style Check strike-throughs, `#`/`##` hanging | `# CHAPTER I` / `## Down the Rabbit-Hole` / RABBIT-HOLE-SUMMARY with `too` (×2) and `very` struck; bright sentence `She follows it down a rabbit hole when suddenly she falls a long way to a curious hall with many locked doors of all sizes.` |
| ianet-mac-dark-focus-sentence-support.webp | https://static.ia.net/2023/05/macOS-fm-sentence.webp (ia.net/writer/support/editor/focus-mode) | 3104×1610 | Mac, dark, Focus: Sentence, window capture with shadow | RABBIT-HOLE-SUMMARY; bright sentence `She follows it … all sizes.` caret after `sizes.` |
| ianet-mac-dark-focus-paragraph-support.webp | https://static.ia.net/2023/05/macOS-fm-paragraphs.webp | 3104×1610 | Mac, dark, Focus: Paragraph (headings + 2nd paragraph dimmed) | RABBIT-HOLE-SUMMARY |
| ianet-mac-dark-typewriter-support.webp | https://static.ia.net/2023/05/macOS-fm-typewriter.webp | 3104×1610 | Mac, dark, Focus: Typewriter (no dimming, caret line vertically centred) | `# 1. Down the Rabbit Hole` / `## Into the rabbit hole` / (blank) / `Alice is feeling bored and *drowsy* while sitting on the riverbank with her elder sister who is reading a book with no pictures or conversations.` caret at end |
| ianet-mac-light-window-focus-syntax-italic.webp | https://static.ia.net/2023/04/write-ia-writer.webp (support/editor) | 3150×2066 | Full macOS window: traffic lights, sidebar toggle, ‹ ›, title `1. Down the Rabbit Hole.txt`, focus dropdown, ▶. Light, Focus Sentence + Syntax Highlight, italic body (font appears to be Quattro Italic — UNVERIFIED) | `## 1. Down the Rabbit Hole` / (blank) / `*Alice is feeling bored and drowsy while sitting on the riverbank … She finds a small key to a door ~~too~~ small for her to fit through, but through it she sees an attractive garden. She then discovers … written on it in currants.*` / (blank) / `/Images/Alice_Rabbit_Time.png` (chip) / (blank) / `Alice was beginning to get *~~very~~* tired of sitting by her sister on the bank, and of having nothing to do: once or twice she had peeped into the book her sister was reading, but it had no pictures or conversations in it, “and what is the use of a book,” thought Alice “without pictures or conversations?”`. Bright sentence: `She finds a small key to a door too small for her to fit through, but through it she sees an attractive garden.` with finds/fit/sees blue, small/small/attractive brown, but green |
| ianet-mac-light-wikilink-autocomplete-zoom.webp | https://static.ia.net/2023/04/Screenshot-2023-04-29-at-15.47.39.webp | 2686×1778 | Mac, light, zoomed crop: `[[Alic` with grey brackets, blue underlined link text, autocomplete popup | `later by the March Hare and the` / `[[Alic]] many riddles and storie…` / … / `There was a table set out under`. Popup: `Alice.txt / iCloud ▸ Videos`, `Alice international.txt / iCloud ▸ International`, `0. Alice in Wonderland.txt / iCloud ▸ Alice`, `0. Alice in Wonderland.txt / iCloud ▸ Alice 2`, `0. Alice in Wonderland.txt / Documents ▸ Alice`, `4. The Rabbit Sends in a Little Bill.txt / iCloud ▸ Alice 2` (selected, blue, ↩) |
| ianet-win-light-library-outline.webp | https://static.ia.net/2023/06/content-win-1.webp | 2048×1152 | Windows 1.x, light, Library (Documents / iCloud Drive / Dropbox), outline, editor with `##` heading and chip | Editor: `she came upon a low curtain she had not noticed before, and behind it was a little door about fifteen inches high: she tried the little golden key in the lock, and to her great delight it fitted!` / `/Images/Alice_Curtain_Key.png` / `## Door to a small passage` / `Alice opened the door and found that it led into a small passage, not much larger than a rat-hole: she knelt down and looked along the passage into the loveliest garden you ever saw. How she longed to get out of that dark hall, and wander about among those beds of bright`. Outline: `1. Down the Rabbit Hole / Into the Rabbit Hole / A Deep Well / Falling Right / After The Fall / Door to a small passage / Drink Me / Eat Me / 2. The Pool of Tears / The White Rabbit Returns / Poem of the Little Crocodile / Splash! / The Mouse / Speaking to the Mouse / Time to Go / 3. A Caucus-Race and a Long Tale / Sit Down! / A Race-Course / Eat the Comfits / 4. The Rabbit Sends in a Little Bill` |
| ianet-mac-settings-general.webp | https://static.ia.net/2023/06/general.webp | 2048×1189 | Mac Settings › General | `Appearance: ☐ Match system appearance ◉ Light ○ Dark; Dock icon: ☐ Match app appearance ◉ Light ○ Dark; File extensions: ☑ Show file extensions; Window: ☐ Always resize window to show Library ☑ Swipe to show Library or Preview ☑ Scroll Preview in sync with Editor; Title bar: Fade In/Out; Toolbar: Fade In/Out; Automation: ☐ Enable URL commands [Manage…]` |
| ianet-mac-settings-markdown-processing.webp | https://static.ia.net/2023/11/SmartPunctua2.webp | 2048×1093 | Mac Settings › Markdown › Processing | `☑ Apply smart punctuation — Convert straight quotes and plain dashes into smart counterparts for all formatted output. ☐ Single return starts a new paragraph — By default, Markdown requires two returns (an empty line) between paragraphs…` |
| ianet-mac-dark-editor-preview-blockquote.webp | https://static.ia.net/2023/05/markdown.webp | 2048×1152 | Mac dark editor + light preview; blockquote `>` hanging in margin, content-block chip | `# CHAPTER II.` / `## The Pool of Tears` / `> Alice is growing to such a tremendous size that her heads hits the ceiling` / `john tenniel alice.png` / `“Curiouser and curiouser!” cried Alice (she was so much surprised, that for the moment she quite forgot how to speak good English); “now I’m opening out like the largest telescope that ever was! Good-bye, feet!” (for when she looked down at her feet, they seemed to be almost out of sight, they were getting so far off). “Oh, my poor little feet, I wonder who will put on your shoes and stockings for you now, dears? I’m sure I shan’t be able! I shall be a great deal too far off to trouble myself about you: you` |
| ianet-mac-light-stats-bar.webp | https://static.ia.net/2023/06/stats.webp | 2048×1152 | Mac light, bottom stats bar + its context menu | `went Alice like the wind, and was just in time to hear it say, as it turned a corner, \`Oh my ears and whiskers, how late it's getting!' She was close behind it when she turned the corner, but the Rabbit was no longer to be seen: she found herself in a long, low hall, which was lit up by a row of lamps hanging from the roof.` / `There were doors all round the hall, but they were all locked; and when Alice had been all the way down one side and up the other, trying every door, she walked sadly down the middle, wondering how she was ever to get out again.` Bar: `62.476 Characters 50.637 Without Spaces 12.072 Words 772 Sentences 01:00:21 Reading Time`; menu ✓Characters ✓Characters Without Spaces ✓Words ✓Sentences ✓Reading Time ✓Tasks › Default / ✓Stats Only |
| ianet-mac-light-editor-preview-split.webp | https://static.ia.net/2023/06/PreviewScreen.webp | 2048×1152 | Mac light split editor/preview, italic emphasis block, `<!--more-->` comment dimmed | `## 7. A Mad Tea-Party` / `*Alice becomes a guest at a "mad" tea party along with the March Hare, the Hatter, and a very tired Dormouse who falls asleep frequently, only to be violently woken up moments later by the March Hare and the Hatter. <!--more-->The characters give Alice many riddles and stories, including the famous "Why is a raven like a writing desk?'. The Hatter reveals that they have tea all day because Time has punished him by eternally standing still at 6 pm (tea time). Alice becomes insulted and tired of being bombarded with riddles and she leaves claiming that it was the stupidest tea party that she had ever been to.*` / `There was a table set out under a tree in front of the house, and the March Hare and the Hatter were having tea at it: a Dormouse was sitting between them, fast asleep, and the other two were using it as a cushion, resting their elbows on it, and talking over its head. "Very uncomfortable for the Dormouse," thought Alice; "only, as it's asleep, I suppose it doesn't mind."` / `### At the Large Table` / `#### Have Some Wine` |
| ianet-mac-light-syntax-highlight-italic.webp | https://static.ia.net/2025/09/Writer-Mac-Syntax-Highlight-Display.webp | 1581×932 | Mac light, Syntax Highlight (adjectives + adverbs only), italic body (Quattro Italic? UNVERIFIED) | `## Alice's Evidence` / `*Alice is then called up as a witness. She accidentally knocks over the jury box with the animals inside them and the King orders the animals be placed back into their seats before the trial continues. The King and Queen order Alice to be gone, citing Rule 42 ("All persons more than a mile high to leave the court"), but Alice disputes their judgement and refuses to leave. She argues with the King and Queen of Hearts over the ridiculous proceedings, eventually refusing to hold her tongue. The Queen shouts her familiar "Off with her head!" but Alice is unafraid, calling them out as just a pack of cards; just as they start to swarm over her. Alice's sister wakes her up from a dream, brushing what turns out to be some leaves and not a shower of playing cards from Alice's face. Alice leaves her sister on the bank to imagine all the curious happenings for herself.*` purple: then, accidentally, back, more, eventually, as just, just, not; brown: high, ridiculous, familiar, unafraid, curious |
| ianet-mac-light-syntax-plus-focus-sentence.webp | https://static.ia.net/2025/09/Writer-Mac-Syntax-Highlight-Focus-Mode-On.webp | 1581×932 | Same passage, Focus Sentence on (only the active sentence keeps colour) | active: `The King and Queen order Alice to be gone, citing Rule 42 ("All persons more than a mile high to leave the court"), but Alice disputes their judgement and refuses to leave.` |
| ianet-mac-light-focus-dropdown-menu.webp | https://static.ia.net/2023/07/EnableFocus.webp | 1537×865 | Mac title bar with file `12. Alice's Evidence.txt` and the Focus dropdown open | Menu: `Disable Focus Mode / ✓ Sentence / Paragraph / Typewriter / Show Syntax: – Adjectives (brown) – Nouns (red) – Adverbs (purple) – Verbs (blue) – Conjunctions (green) / Enable Style Check: – Fillers – Clichés – Redundancies / Custom` |
| ianet-ios-light-focus-paragraph-stylecheck.webp | https://static.ia.net/writer/writer-alice-174d9e.webp | 1530×3036 | iPhone, light, Focus Paragraph, Style Check, keyboard accessory bar (search, ◂ ▸, keyboard, undo, redo, ⌘) | `…found herself in a long, low hall, which was lit up by a row of lamps hanging from the roof.` / `There were doors all round the hall, but they were all locked; and when Alice had been all the way down one side and up the other, trying every door, she walked sadly down the middle, wondering how she was ever to get out again.` (bright, caret) / `Suddenly she came upon ~~a little~~ three-legged table, all made of solid glass: there was nothing on it but a tiny golden key, and Alice’s first idea was that this might belong to one of the doors of the hall; but, alas! either the locks were ~~too~~ large, or the key was ~~too~~ small, but at any rate it` |
| ianet-mac-light-library-organizer-filelist.webp | https://static.ia.net/2024/03/orgfile.webp | 2048×1152 | Mac light, annotated `Organizer` / `File List` panes | Organizer: `Locations / ☁ iCloud / Favorites / Smart Folders / Hashtags`; list `Alice`, `Sort by Date Created`: `0. Alice in Wonderland.txt Yesterday / Alice in Wonderland Alice/1. Down the Rabbit Hole.txt The Pool of Tears.txt`; `1. Down the Rabbit Hole.txt 2022/08/23 / Down the Rabbit Hole Into the rabbit hole Alice is feeling bored and drowsy while`; `2. The Pool of Tears.txt 2020/01/16 / The Pool of Tears *Alice is growing to such a tremendous size her head hits the`; `12. Alice's Evidence.txt 2022/10/12 / Alice's Evidence *Alice is then called up as a witness. She accidentally knocks over`; `10. The Lobster Quadrille.txt 2018/02/18 / The Lobster Quadrille The Mock Turtle and the Gryphon dance to the Lobster`; `9. The Mock Turtle's Story.txt 2022/09/14`. Editor: `## 4. The Rabbit Sen…` / `*The White Rabbit … gloves and fan. M… he orders Alice t… once she gets ins… orders his garden… go down the chim… animals that have… hurls pebbles at … them, and they re…` |
| ianet-mac-light-metadata-editor-preview.webp | https://static.ia.net/2023/06/metadata-mac-1.webp | 2048×1152 | Mac light split, YAML metadata block | `---` / `Customer: M. Bluth` / `Me: Bob Loblaw` / `Date: April 3rd, 2023` / `---` / `Dear [%customer],` / `Thank you for your order.` / `It has been shipped Irom our warehouse and you can expect the delivery on [%date].` / `Sincerely,` / `[%me]` (caret). Preview: `Dear M. Bluth, / Thank you for your order. / It has been shipped Irom our warehouse and you can expect the delivery on April 3rd, 2023. / Sincerely, / Bob Loblaw` |
| ianet-mac-dark-authorship-paste-as.webp | https://static.ia.net/2023/12/authorship.webp | 2210×1364 | Mac dark window `Authorship.txt — Edited` over a Safari ChatGPT window; context menu `Cut / Copy / Paste / Paste As ▸ (Me / ✓ AI / New Author…) / Format ▸` | `*Authorship* separates the text you typed and the text you pasted from AI. By dimming the text you pasted from other sources, you make sure that your voice is what comes through on the page.` / `As the train passed over the bridge, the rhythmic clatter of wheels against the tracks echoed through the misty valley, painting a scene of serene motion against the backdrop of the rising sun. ` (dimmed, caret) |
| ianet-devices-dark-editor-mockup.png | https://static.ia.net/writer/landing/threedevices2x.png | 1988×988 | Device mockup (MacBook, iPhone, iPad) all dark, Focus Sentence — marketing, tiny UI | ALICE-FALL |

Download notes / failures:
* `…@2x.png` variants of the ia.net landing PNGs return 404 (the plain names are already the large originals).
* `apps.apple.com` HTML is blocked for curl (959-byte stub); the iTunes Lookup API (`itunes.apple.com/lookup?id=…`) returned the screenshot URLs, and rewriting the thumbnail suffix to `5120x3200bb.png` yields the 2560×1600 originals (they cap at the upload size). Mac listing = id775737590, iOS listing = id775737172. iPhone 05–10 and iPad 03–10 were downloaded to /tmp but are duplicates of the Mac slide set and were not copied.
* `apps.microsoft.com` HTML contains no image URLs; the StoreEdge API (`storeedgefd.dsx.mp.microsoft.com/v9.0/products/XPFM5GFVD6WS74`) returned eight 1440×900 screenshots (product last updated 2022-09-21 per API — Windows 1.x/2.x era UI).
* ia.net/writer/mac, /windows, /ios, /android are thin redirect-style pages (≈6.7 KB) with only award badges; all real product images live on ia.net/writer and the support pages.
* ia.net/topics/duospace and /topics/ia-writer-mono-duo-quattro do not exist (404); the real posts are listed in §3.

### 1.1 Captures of the app itself (`owner-mac-*`, `mac-native-*`)

Everything above is a marketing frame or a support-page image. What follows is iA Writer **running natively on macOS**, captured on the owner's own machine. These outrank a marketing frame wherever the two disagree — three claims elsewhere in this repo were read off marketing frames and all three were wrong (ADRs 0013 and 0014).

**The first four (`owner-mac-*`)**, taken 2026-08-30, dark theme. [#154](https://github.com/danielbaldwin47/Quill/issues/154) collects the systematic set; these are the first four.

| File | px | What it shows | Measured |
|---|---|---|---|
| owner-mac-01-dark-caret-midword.png | 1090×124 | caret between two letters of `test` | one bar `#00bfff`, 6 × 63 px, x 331–336 |
| owner-mac-02-dark-caret-line-end.png | 1224×162 | caret past the last glyph of the line | one bar `#00bfff`, 6 × 63 px, x 1120–1125 |
| owner-mac-03-dark-selection-inline.png | 1146×124 | `caret and selection` held, inside one row | fill `#113d52`, x 594–1071, band 60 px tall; **no `#00bfff` pixel in the frame** — no end bars and no caret |
| owner-mac-04-dark-selection-multiline.png | 2140×328 | a selection running from mid-line through a heading and a quote block | one contiguous fill band, y 44–259; **no `#00bfff` pixel in the frame** |

The two selection frames are what ADR 0014 rests on, and they are read the safe way round: a caret bar is the brightest thing in any frame that holds one, so a frame with no accent pixel in it holds neither a bar nor a caret. Note that `appstore-mac-04` above — the frame ADR 0012 read as the Mac app's selection — shows 44-px round touch grab-handles, an affordance macOS does not have.

**The systematic set (`mac-native-*`)** — 76 captures of the same app driven state by state for [#154](https://github.com/danielbaldwin47/Quill/issues/154), and the evidence every corrected number below is read from. They outrank every marketing frame the same way the four above do.

| | |
|---|---|
| App | iA Writer **8.0.6** (build 80046, `pro.writer.mac`) |
| OS | **macOS 27.0** (26A5406e) |
| Backing scale | **2.0** (NOTES.md § The rig carries the verification) |
| Window | the same size for every state — **3024 × 1898 device px** (NOTES.md § The rig has the logical rect it was set from) |
| Typeface | **Mono** throughout, so a cell pitch can be fitted (Duo is duospaced and no single advance fits it) |
| Off for every state | Style Check, Syntax Highlight, Authors; Focus Mode too, except states 13 and 15 |

**Every `mac-native` number quoted below is in device pixels at backing scale 2.0** — divide by two for logical points. Captures are cited below by the short names VERDICTS.md uses — `12-light`, `11-00 … 11-13`, `14-gutters`, `09-select-all` — which are abbreviations, not a prefix rule: NOTES.md names the file each one stands for, state by state.

| Where | What |
|---|---|
| `ref/ia/shots/mac-native/` | the 76 PNG captures |
| `ref/ia/mac-native/NOTES.md` | how each state was driven and what it measured, state by state |
| `ref/ia/mac-native/VERDICTS.md` | every claim in §§3.5, 4.1, 4.2 and in ADRs 0012–0014, marked confirmed / contradicted / still unknown against the running app, each naming its capture |
| `ref/ia/mac-native/rig/` | the driver that took them (`iarig.py`, `drv.sh`, the per-state scripts) |
| `ref/ia/mac-native/blink-idle.tsv`, `blink-typing.tsv` | the ≈103 Hz blink samples behind §3.5's cadence |
| `ref/ia/mac-native/passage-blocks.md`, `passage-markers.md` | the two passages `ref/sample.md` has not got |

---

## 2. Fonts (`fonts/`)

Source: https://github.com/iaolo/iA-Fonts (commit f32c04c3058a75d7ce28919ce70fe8800817491b, 2023-06-16). Copied per family: 4 static TTFs (Regular, Italic, Bold, BoldItalic), 4 WOFF2, 2 variable TTFs (upright + italic), LICENSE.md, Readme.md.

| Family | Static TTF | WOFF2 | Variable |
|---|---|---|---|
| Duo | iAWriterDuoS-Regular.ttf, iAWriterDuoS-Italic.ttf, iAWriterDuoS-Bold.ttf, iAWriterDuoS-BoldItalic.ttf | same names .woff2 | iAWriterDuoV.ttf, iAWriterDuoV-Italic.ttf |
| Quattro | iAWriterQuattroS-Regular/Italic/Bold/BoldItalic.ttf | same .woff2 | iAWriterQuattroV.ttf, iAWriterQuattroV-Italic.ttf |
| Mono | iAWriterMonoS-Regular/Italic/Bold/BoldItalic.ttf | same .woff2 | iAWriterMonoV.ttf, iAWriterMonoV-Italic.ttf |

License (fonts/*/LICENSE.md): "Copyright © 2018 Information Architects Inc. with Reserved Font Name "iA Writer" / Based on IBM Plex Typeface, Copyright © 2017 IBM Corp. with Reserved Font Name "Plex" / This Font Software is licensed under the SIL Open Font License, Version 1.1." Repo README also says: "If you fork or use our fonts, please reference iA Writer clearly. Use them creatively. Don't be a copycat. With or without the fonts, do not clone our products or our website."

Font metrics (parsed from the TTF tables with a hand-written parser; `unitsPerEm = 1000` for all):

| | Mono | Duo | Quattro |
|---|---|---|---|
| Advance widths present (Regular) | 600 only (plus 601/667/694/832 for a few non-Latin glyphs) | 300, 600, 900, 1200 | 300, 450, 550, 600, 750, 900, 1200 |
| i, l | 600 | 600 | **300** |
| n, a, ., space | 600 | 600 | 600 / 600 / 600 / **450 (space)** |
| m, w, M, W | 600 | **900** | **900** |
| hhea ascent / descent / lineGap | 1025 / −275 / 0 → natural line = 1.30 em | same | same |
| OS/2 typoAsc / typoDesc / typoGap | 780 / −220 / 300 (USE_TYPO_METRICS off) | same | same |
| x-height / cap-height | 516 / 698 | same | same |

So: Duo = monospace at 0.6 em with m/w/M/W at 1.5 cells (0.9 em); Quattro additionally narrows i/l to 0.5 cell and the space to 0.75 cell. A "64-character line" of Duo is 64 × 0.6 em = 38.4 em wide (before the 1.5-cell exceptions).

---

## 3. Typography, focus, chrome — iA's own statements (verbatim)

### 3.1 Which font is default, and why

* iA Writer (official X account, 2023-03-09, https://x.com/iAWriter/status/1633947520823881730, text retrieved via fxtwitter): "A monospaced font has always been available in iA Writer. Duo (duospaced) is the default, but you can switch to Mono in Editor settings."
* Settings page (https://ia.net/writer/support/basics/settings): "Typeface — Select the font between iA Mono, Duo or Quattro."
* Windows settings (https://ia.net/writer/support/basics/settings/settings-windows): "Font — Choose between iA Writer Mono, Duo, or Quattro fonts."
* Mac version history 5.2 (https://ia.net/writer/support/help/version-history): "Deep Typography Refresh — New typeface in three flavors — Mono: Single character width, the classic — Duo: Two widths, freeing M and W — Quattro: Four widths, for a cleaner text image — Over 1,000 optical variations seamlessly adapt to provide the best writing experience in any environment".
* "In Search of the Perfect Writing Font" (2017-11-23, https://ia.net/topics/in-search-of-the-perfect-writing-font):
  * "In contrast to proportional fonts that communicate "this is almost done" monospace fonts suggest "this text is work in progress." It is the more honest typographic choice for a text that is not ready to publish."
  * "In a monospace font every letter, every number, every punctuation mark and every space takes the same visual space, which slows us down. And, for writing that's a good thing."
  * "A typical proportional font comes with word spaces as wide as an i. Monospace fonts come with rather large word spaces. This makes it easier to discern each word and letter."
  * "Duospace is a notion familiar from Asian fonts where there are single and double width characters. Our candidate is a bit different. It offers single and four 1.5 width characters."
  * "Duospace gives 50% more space to the letters m, M, w and W. It takes two of those to get back in step with the monospace rhythm. The advantage over proportional fonts is that you keep all benefits of the monospace: the draft like look, the discernability of words and letters, and the right pace for writing."
  * "We adjusted the upper and lower case M's and W's as we did in iA 735, adjusted the g and, here it was … we kept the one-storey lowercase g. … for writing purposes, the single story g makes the text image more homogenous, calmer."
* "A Typographic Christmas" (2018-12-14, https://ia.net/topics/a-typographic-christmas): "Quattro shares similarities with a proportional typeface. At the same time, it retains a lot of the technical virtues of the classic typewriter fonts using wider gaps between the words and giving each letter more room than a classic, fully proportional face." / "We kept large word spacing and monospaced punctuation. Quattro saves space on small screens." / "Using square dots instead of round ones, adjusting the swirls and curves on a, j, f, l, t, y, Q, and lots of non-latin characters, we unified mono, Duo, and Quattro under a common look."
* "Responsive Typography: The Basics" (2012-06-01, https://ia.net/topics/responsive-typography-the-basics): "For iA Writer we chose a monospace typeface. Because the primary purpose of our program is helping you getting a first draft out, we specifically chose Nitti—a typeface that feels strong and careful at the same time. The decision to use a monospace typeface also came about because the first iPad's Operating System didn't auto-kern proportional typefaces."

### 3.2 Font size, line height, measure, margins

* Settings (Mac): "Text size — Adjust slider to desired text size." Shortcuts: "⌘+ Make text larger / ⌘- Make text smaller / ⌘0 Default text size". No numeric default is published. UNVERIFIED default point size — see measured values in §4.
* Settings (Mac): "Line length limit — Select maximum characters per line (64, 72 or 80)."
* Settings (Windows): "Line Length — The default width allows for 64 characters on a single line before it wraps."
* Windows 2.0 post (2024-12-11, https://ia.net/topics/ia-writer-for-windows-2-0): "the same dynamic type engine as our apps on Apple platforms with liquid font weight, width and line spacing adjustments" — i.e. weight, width and line spacing are functions of text size (the "Over 1,000 optical variations" of 5.2). Exact curve not published (UNVERIFIED).
* "Responsive Typography: The Basics": "optimal readability requires a certain amount of control over the measure (column width) of the text" / "With more reading distance and (what we call) pixel smear, it's wise to give screen text a little bit more line height than printed text. 140% is a good benchmark, but of course, it depends on the typeface you use." / "Some people complain about the big font size in Writer for Mac. … we went for the biggest minimal font size on Mac as well. At the time our benchmark was a 24 inch high resolution iMac, where the perceived size is more or less the same as on all other devices."
* Templates README (https://github.com/iainc/iA-Writer-Templates): "iA Writer will adjust the `<html>` element padding in Preview to match Editor" and "Avoid vertical margins and padding for the document page." (Editor margins are therefore app-computed, not CSS; no numbers published.)
* Paragraph spacing: Settings › Markdown: "SINGLE RETURN: Traditional Markdown requires two returns to complete a paragraph. i.e., one empty line between paragraphs or any other element." The editor shows the blank line literally (measured: paragraph gap = exactly 2 line pitches, §4).

### 3.3 Focus Mode / Typewriter

Support page (https://ia.net/writer/support/editor/focus-mode):
* "Focus mode for Mac emphasizes the active sentence or paragraph while eliminating distractions, so you can concentrate on the current task."
* "According to user feedback, this feature shines at best when used in full-screen, dark mode."
* Enable via "the Menu Focus → Enable Focus Mode / the keyboard shortcut ⌘D / the Title Bar button". "Then, you will have to choose among 3 different settings: Sentence, Paragraph, and Typewriter."
* "Sentence — The active sentence –where your cursor is placed– is highlighted while the surrounding text is dimmed."
* "Paragraph — The whole active paragraph is highlighted while the surrounding text is dimmed. Moving the cursor to a different paragraph will make it an active paragraph."
* "Typewriter — Unlike the two previous options, the text will not be highlighted or dimmed in this mode. The cursor remains vertically centered in the Editor when typing or moving up or down in your document. The experience is similar to what you would have with a mechanical typewriter."
* "A conflict between the area you will select to edit and Focus Mode's attempt to vertically center the cursor might result in the screen jumping vertically." (implies Sentence/Paragraph focus also keep the caret vertically centred)
* Windows (https://ia.net/writer/support/editor/focus-mode/focus-mode-windows): shortcut "CtrlShiftD (sentence mode)"; "Typewriter Scrolling can be enabled independently of other Focus Mode or Syntax Control options."
* How-to (https://ia.net/writer/how-to/write-with-focus): "With Focus Mode enabled, you'll find that text in the Editor goes gray. Only the sentence or paragraph where your cursor is located stays bright." / "The Typewriter option makes the Editor behave more like (yes, you guessed it) a mechanical typewriter by always vertically centering the Editor to wherever your cursor is."
* History: Mac 3.0 "Typewriter Mode (⌘T)"; 3.1.3 "Focus Mode can be expanded to the bounds of full paragraphs"; later "Added toggle button for Focus Mode in title bar / Focus Mode now dismisses Preview and Library panes".
* Dimming amount/colour: not published. MEASURED against the running app: dimmed text **#c6c4c2** on light paper **#f7f7f7**, **#707070** on dark **#1a1a1a** (`13-light-sentence`, `13-dark-sentence`, `12-light`, `12-dark`). *The light pair was read as ≈ #cccccc on #f9f9f9 from App Store 06 and the App Store PNG; both are superseded readings (VERDICTS 4.2.1, 4.2.5).*
* MEASURED: there is **one dim tier per theme**, not several — Sentence and Paragraph dim to the same colour and differ only in how much stays lit (`13-*`; VERDICTS 4.2.12). **Typewriter is a third value of the same "Focus scope" popup**, not an independent toggle (Settings → Editor), and it holds the caret's line at the window's vertical centre — mid-point 948.0 of 1898 = 49.9 % (`15-dark`).

### 3.4 Dark mode

* Version history: "Renamed night mode to dark appearance to conform with a future update of macOS / System light and dark appearance state is matched by default with a future update of macOS".
* Settings › General: "Appearance — Choose between Light and Dark themes for iA Writer." Screenshot shows `☐ Match system appearance ◉ Light ○ Dark`.
* Templates: "Invert Colors: uses Night Mode when the Editor is in Day Mode and vice-versa." Templates README: "`night-mode` — Night mode is active." / "default templates have a slightly different color scheme on each platform."
* 2019-10-31 (https://ia.net/topics/multi-window-dark-mode-content-block-flexibility): "iA Writer has supported night mode for a long time. We now also allow you to automatically match the color scheme to the appearance you use in your system settings."
* App Store description: "iA Writer includes an inverted light-on-dark-mode, perfect for working day and night."
* Palette hex values are NOT published by iA. Measured values in §4.

### 3.5 Caret

* No official spec published. Secondary quote surfaced by search (attributed to iA, source page not retrievable — UNVERIFIED): iA "invested significant energy into making a blue wider caret at a time when all text views had black hairline carets, which is why the app identifies so strongly with that blue caret." Design Takes Time (https://ia.net/topics/design-takes-time): "Currently, iA Writer uses the letters "iA" and a caret symbol as its basic elements. The caret represents the word "Writer"." and "We tested the idea using a gradient cursor in the app. In practice people found it to be mostly irritating."
* Measured against the running app (§4.1, §4.2): the caret is a single flat **#00bfff**, 220 flat px of it, identical in both themes (`01-dark`, `01-light`); its **height is the full line pitch at every one of the app's 14 text sizes**, within the 1 px two roundings can differ (`11-00 … 11-13`); its width **quantises to 5 / 6 / 8 / 10 px** and is therefore *not* a constant fraction of the em — 0.141 em at the default, 0.080–0.186 em across the range (`11-00 … 11-13`); no visible rounding — the end is square, with 1 px of corner antialiasing over the top and bottom two rows only (`01-dark`). *Was read as blue #00b5ff–#01c4ff, width ≈ 0.12–0.17 em from the App Store and ia.net stills: the blue is contradicted (VERDICTS 4.2.7, 4.2.8) and the width band holds near the default only (VERDICTS 3.5.5).*
* Blink, measured (`04-f00 … f12` with `blink-idle.tsv`; `05-*` with `blink-typing.tsv`): a **1.000 s period, 0.516 s on / 0.484 s off** at full strength, with ≈ 0.09 s fade ramps between. The blink is **suppressed while typing** — solid on for 3.448 s of continuous typing — and resumes **0.633 s** after the last keystroke. *Was "not visible in stills (UNVERIFIED; standard platform blink presumed)".*
* Measured: the caret is **removed entirely while a selection stands** (`07`–`10`, both themes, all four selection shapes) and **removed when the window deactivates** — 0 accent pixels across a 14-frame burst in both themes, with the editor's own ground filling the frame, so the window was visible and simply had no bar in it (`06-dark`, `06-light`).

### 3.6 Markup / syntax rendering in the editor

iA publishes no styling spec; the following is OBSERVED in the screenshots above:
* Headings `#…######`: same size and colour as body, **bold**; the hash marks plus the following space hang in the left margin so the heading text aligns with the body column (measured: `#` starts exactly 2 cells left of the body edge, `##` 3 cells, `####` 5 cells).
* Blockquote `>`: MEASURED against the running app — the `>` does **not** hang: it sits on the body column (695 against the column's 691), and list `-` and ordered `1.` do not hang either (`14-blocks`; VERDICTS 4.1.13). *Was read from markdown.webp as hanging 1 cell + space.*
* Emphasis `*…*`: the asterisks are kept, text rendered in the font's Italic; `**…**` bold (Markup examples: `*never*`, `*drowsy*`, `*Authorship*`).
* Strikethrough (Style Check) and `~~…~~`: dimmed (#707070 on dark) with a line through.
* Wikilinks `[[…]]`: brackets in dim grey (#707070 on dark), link text in the caret blue (**#00bfff**, VERDICTS 4.2.7) with underline. *The link colour was read as #00b5ff from a marketing still; the running app's accent is #00bfff, though wikilinks themselves were not among the `mac-native` states (VERDICTS 4.2.13).*
* Content blocks (`/Images/x.png`, `alice.jpg`): rendered as a rounded "chip" — white (#ffffff) fill with a 1-px #e1e1e1 border on the light theme; on dark, a slightly lighter box with a blue outline while focused.
* HTML comment `<!--more-->` and `//` comments: dimmed grey.
* YAML front matter `---`: rendered as plain text in the editor, hidden in preview.
* Syntax Highlight (parts of speech) — support page (https://ia.net/writer/support/editor/syntax-highlight): "Adjectives in brown / Nouns in red / Adverbs in purple / Verbs in blue / Conjunctions in green". Exact light-theme values measured from the lossless PNG: blue #3476b9, brown #a66500, purple #b24fa2, green #3f831e; red (nouns) ≈ #ca471a (from the App Store slide headline that reuses the palette — UNVERIFIED as the exact editor value). Dark-theme values (App Store 03, lossless): verbs #799fc2, adjectives #c1934e, adverbs ≈ #ba8eb2.
* Authorship (dark): AI/other-author text dimmed to #707070; per-author colours observed ≈ #faa25d (orange), #fb8b65, #fc686f (red-pink), #d177ce / #de71b2 (purple), blue ≈ **#00bfff**-family (VERDICTS 4.2.7). *The blue was read as an #00b5ff family from a marketing still; author colours were not covered by the `mac-native` states, which hid Authors (VERDICTS 4.2.15).*
* John Gruber, quoted on ia.net/writer: "the gold standard for Markdown syntax styling — great colors, real italic and bold styling for _italic_ and **bold** spans".

### 3.7 Minimal-chrome philosophy

* ia.net/writer: "has fewer features, by design. But each one is intentional, built for focus, and made to help you write better. Nothing distracts, nothing decorates." / "iA Writer separates writing from formatting. So you can focus on one at a time." / "Imagine a place where all you can do is write."
* Settings › General: "Titlebar — Choose if you always see the Title bar or if it fades in/out. / Toolbar — Choose if you always/never see the Toolbar or if it fades in/out." Windows: "Auto Hide TitleBar — When enabled TitleBar hides when you start typing or scrolling within a document." / "Hide Taskbar on Maximize — Check this option to get the true full-screen experience."
* Windows 2.0 post footnote: "Features can distract and get in the way. If they slow down the app they will slow down your thoughts, and eventually, they'll bring your writing to a screeching halt."
* Press quotes reproduced on ia.net/writer: "The program is, essentially, a white rectangle, where the user can do little else but type in a custom monospaced font. There are no headers, footers, drawing tools, or chatty paper-clip assistants."; "little on-screen clutter like buttons and controls. The app borrows the look and feel of its writing interface from a traditional typewriter, including the font you type in."
* Observed chrome (screenshots): title bar = centred file name with document icon; left: sidebar toggle + ‹ › history; right: focus-mode dropdown (list icon + ⌄), search, preview ▶. Optional bottom stats bar. Everything else is the text column.

### 3.8 Keyboard shortcuts (Mac, https://ia.net/writer/support/basics/shortcuts)

* Focus: `⌘D` Enable or disable Focus Mode · `⇧⌘D` Syntax Highlight · `⌥⇧⌘D` Style Checking · (historic `⌘T` Typewriter Mode, v3.0)
* Interface: `⌃⌘N` Dark or light appearance · `⌃⌘S` Show or hide Library · `⌘R` Show or hide Preview · `⌃⌘F` Enter or exit full screen · `⌘W` / `⌥⌘W` close window / all
* Text size: `⌘+` larger · `⌘-` smaller · `⌘0` default
* Structure: `⌘1–6` Heading 1–6 · `⌥⌘L` task list · `⇧⌘L` ordered list · `⌥⌘X` mark task completed · `⇥`/`⇧⇥` indent/outdent · `⌥⌘↑/↓` move line
* Formatting: `⌘B` bold · `⌘I` italic · `⌥⌘U` strikethrough · `⇧⌘U` highlight · `⌘K` link · `⌥⌘K` image link · `⇧⌘K` wikilink · `⌃⌘K` footnote · `⌘J` code · `⇧⌘J` code block
* Files: `⌘N` New in Library in current window · `⌥⌘N` New · `⇧⌘N` New in Library · `⌘S` save · `⇧⌘S` duplicate · `⇧⌘O` Quick Search · `⌃⌘←/→` previous/next document · `⌃tab` cycle Organizer → File List → Editor · `⌘,` Settings
* Caret: `⌥⌘←/→` start/end of this or previous/next sentence (in addition to standard macOS word/line/document moves). Windows: `Ctrl Shift D` Focus (sentence).

### 3.9 Library / file model

* App Store: "Search, sort, and quickly swap between documents from different clouds without leaving the window." / "First, write in plain text. Then preview in HTML."
* How-to: "When you create a new file, Writer uses the first line of your text to name it." / "Writer will save your changes every few seconds as you write, then save it once more when you close it."
* Settings › Files: "Default extension — New files will be saved with the extension provided here. Common extensions are .txt, .text or .md for Markdown." / "Preferred action — Determines the behavior of ⌘N; either create files in the currently shown folder of the Library or prompt for a save location upon ⌘S." / "Open files — Enable or disable for "Ask to keep changes" when closing."
* Settings › Library: "Organizer — Choose which sections to display" (Locations, Favorites, Smart Folders, Hashtags); "Files — keep folders at the top … show text excerpts for each file and show or hide the Sort and Filter bars"; "Sort by — modification date, name, or kind."
* Shortcuts page: "“New in Library” files are automatically created in the currently visible folder. “New” files will ask whether you want to save them when closed."
* Features page: iCloud, Dropbox, Google Drive, OneDrive, Other (local folder) on all platforms; Wikilinks on Apple only; Folding/Dynamic Outline/Writing Goals on Windows only.

---

## 4. Spec sheet (numbers)

Legend: **P** = published by iA, **M** = measured from the named screenshot, **U** = unverified. Two methods are behind **M**. Marketing stills were read with ImageMagick projections and histograms, the character advance fitted by least squares over several lines using Duo cell widths 1.0 / 1.5. The `mac-native` captures (§1.1) were read off the running app, and their cell advance comes from the difference between two selection fills of known cell counts, which needs no assumption about the face; where the two disagree the `mac-native` value is the app's.

### 4.1 Geometry

Every row marked `mac-native` is measured off the running app (§1.1) and names its capture. Where a row corrects an earlier reading, the earlier reading is kept in the row as what it was and where it was read from, so the correction can be seen rather than inferred. Full verdicts: [VERDICTS.md § 4.1](mac-native/VERDICTS.md).

| Item | Value | Basis |
|---|---|---|
| Line-length limit | 64 / 72 / 80 characters; **64 is the default** | P (Mac & Windows settings pages), and confirmed in the running app: the Editor settings popup offers exactly those three with 64 checked |
| Default text size | **em 42.67 px = 21.33 pt**. The app offers **14 sizes**, em 14.50 pt … 62.58 pt (ladder below); the default is step 5 | M `11-00 … 11-13`. *Was "not published", with three sampled stills at 54.3 px, 41.3 px and 49.5 px — those are the stills' own scales, and none of them is the app's default* |
| Line height | **liquid, and a falling curve, not scatter**: `pitch / em` runs **1.732 → 1.374** monotonically from step 2 up, and is **1.711** at the default (73 px pitch on a 42.67 px em). The font's own hhea line height is 1.30 em, so the app's added leading is **+0.41 em at the default but +0.07 em at the largest size** — the "≈ +0.3–0.5 em" band describes the small end alone | M `11-00 … 11-13` + font tables. *Was 1.71× at 54 px, 1.77× at 41 px and 1.62× in appstore-mac-09, read as three unrelated samples* |
| Paragraph spacing | blank Markdown line = exactly one empty line: the paragraph break is **145–146 px = 2 × the 73 px pitch**. No extra paragraph margin. Headings sit on the same grid as body lines — heading-to-heading pitch **74 px** against body's 73, the 1 px being the taller line box | M `14-gutters`. *Was 187 px and 221 px, measured on stills of unknown scale* |
| Text container | the 64-cell measure **plus a 7-cell hanging-marker gutter on each side — 78 cells** — centred in the window: x 512 … 2511, centre **1512.0** of 3024. The 7 cells are exactly what the deepest heading needs: `###### ` hangs 6.95 cells, to x 513, the container's own left edge | M `09-select-all`, `14-gutters` |
| Text column (the measure itself) | 64 cells = 38.4 em: x 691.0 … 2329.4 = **1638.4 px = 64 × 25.6** | M `08-cells-05/10/20/40`. *Was read off two marketing stills as "widest line 2112 px on a 3234-px-wide capture (65 %); left text edge x=531 of 3234 (16.4 %)", with the centring inferred from a 2880 px-wide dark capture whose column started 577 px in and would end ≈716 px from the right — "approximately centred, the difference being the hanging-marker gutter". The gutter is now measured, at 7 cells a side, and the centring is exact* |
| Character advance | **Mono: 0.6 em exactly** — a 25.6 px cell on a 42.67 px em, matching the font's own 600/1000 | M `mac-native` grid section + font tables. *The old row measured **Duo** at 0.6 em (32.56 px at 54.3 px, 24.8 px at 41.3 px) from the font tables and two stills, and that reading is untouched: Duo is **not covered** by the `mac-native` states, every one of which was shot in Mono so that a cell pitch could be fitted (VERDICTS 4.1.7)* |
| Hanging markers | `#`+space = 2 cells, `##` = 3, `###` = 4, `####` = 5, `#####` = 6, `######` = 7 — the ladder is **(level + 1)**, measured 1.95 / 2.97 / 4.96 / 6.95 cells. **`>` does not hang at all**: it sits on the body column, 695 against 691, and list `-` and ordered `1.` do not hang either | M `14-gutters`, `14-blocks`. *`>` "= 2 cells" was read from markdown.webp* |
| Caret | **6 × 72 px at the default** (em 42.67 px against a 73 px pitch). Height is the line pitch at every size within 1 px; width **quantises to 5 / 6 / 8 / 10 px** across the 14 sizes — see the ladder | M `01-*`, `11-00 … 11-13`. *Was 9 × 93 px at 54.3 px, 6 × 70 at 41.3 px, 7 × 80 in appstore-mac-09 and 5 × 51 on Windows: the stills' own sizes, which this app's ladder does not name* |
| Caret at a line end | **not flush after the last glyph**: at a hard line end the bar's centre is 1229.0 against an advance boundary of 1228.6, leaving **7 px of clear paper** between the bar and the last glyph's ink. The ink ends at 1218 and the glyph's right side bearing is 10.6 px, of which the bar covers only the last 2.6 | M `02-dark`, `02-light`. *Was "sits flush after the last glyph"* |
| Caret against the boundary | the 6 px bar is **centred on the advance boundary, 3 px each side** — centres 691.0 / 819.0 / 1229.0 against boundaries 691.0 / 819.0 / 1228.6, an offset of 0.000 em. It tracks the grid rather than landing on it once: the caret at offsets 0, 10 and 20 of one line has its centre at 691.0 / 947.0 / 1203.0, **strides of exactly 256 px = 10 × 25.6**, against a boundary of 691.0 + 25.6 n | M `01`, `02`, `03`, `01-caret-offset-00/10/20` |
| Selection band | a lone held row is **70 px** against the 73 px pitch; stacked rows abut at the full pitch with **no gap** — sampled down the right gutter, where no glyph can interrupt it, the fill runs 292 px with no internal break, exactly four 73 px pitches. The fill sits **under the ink** — glyphs interrupt the band at their own columns | M `08-dark`, `09-dark` |
| Selection shape across rows | interior rows fill the **whole container** (512 … 2511); the first row runs from the anchor to the container's right edge (920 … 2511), the last from the container's left edge to the focus (512 … 921) | M `09-dark` |
| A held hard newline | fills to the **container's right edge**, not a one-cell stub: x 1228 … 2511, w 1284 | M `10-newline-only` |
| Selection ends | **nothing marks either end** — no bar, no handle, no caret, in either theme at any of the four shapes shot; the fill's own edge is all there is | M `07`–`10` |
| Heading style | bold, same size and colour as body: heading ink `#cccccc` identical to body on dark, the line box 40 px against body's 38, the same for all six levels | M `14-gutters` |
| Rounded window/editor corners | present: all four corners transparent, corner radius ≈ **35 px (17.5 pt)** | M `00-window-chrome` |
| Typewriter | holds the caret's line at the **vertical centre** of the window — mid-point 948.0 of 1898 = 49.9 % — and is a **value of the Focus scope popup**, alongside Sentence and Paragraph, not a separate mode | M `15-dark`, Settings → Editor |

**The 14 text sizes** (`11-00 … 11-13`; device px at backing scale 2.0). `em` is derived from the measured advance at Mono's own 0.6 em per cell, `pt` is `em / 2` at this scale, `band` is the selection band's height. This table replaces the three sampled stills the "default text size" and "line height" rows used to rest on.

| step | cell px | em px | em pt | pitch px | pitch / em | band px | caret w | caret h | caret w / em |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 17.4 | 29.00 | 14.50 | 49 | 1.690 | 46 | 5 | 49 | 0.172 |
| 1 | 18.3 | 30.50 | 15.25 | 52 | 1.705 | 50 | 5 | 53 | 0.164 |
| 2 | 19.4 | 32.33 | 16.17 | 56 | 1.732 | 54 | 6 | 56 | 0.186 |
| 3 | 20.6 | 34.33 | 17.17 | 59 | 1.718 | 58 | 6 | 59 | 0.175 |
| 4 | 23.1 | 38.50 | 19.25 | 66 | 1.714 | 64 | 6 | 67 | 0.156 |
| **5 — Default** | **25.6** | **42.67** | **21.33** | **73** | **1.711** | **70** | **6** | **72** | **0.141** |
| 6 | 30.7 | 51.17 | 25.58 | 86 | 1.681 | 84 | 8 | 86 | 0.156 |
| 7 | 35.7 | 59.50 | 29.75 | 98 | 1.647 | 94 | 8 | 98 | 0.134 |
| 8 | 40.7 | 67.83 | 33.92 | 109 | 1.607 | 108 | 8 | 109 | 0.118 |
| 9 | 45.7 | 76.17 | 38.08 | 120 | 1.575 | 118 | 10 | 120 | 0.131 |
| 10 | 53.1 | 88.50 | 44.25 | 135 | 1.525 | 134 | 10 | 135 | 0.113 |
| 11 | 60.4 | 100.67 | 50.33 | 149 | 1.480 | 146 | 10 | 148 | 0.099 |
| 12 | 67.8 | 113.00 | 56.50 | 161 | 1.425 | 158 | 10 | 161 | 0.088 |
| 13 | 75.1 | 125.17 | 62.58 | 172 | 1.374 | 170 | 10 | 172 | 0.080 |

The wobble across steps 0–3 is whole-pixel quantisation on a 29 px em. Caveat for the last six rows: 78 cells (64 plus two 7-cell gutters) needs 78 × cell px of window, which past step 7 exceeds this window's 3024 px — from step 8 up the column is limited by the window, not by the app's line-length setting. Pitch, advance and caret are unaffected; the measure's own width from step 8 up is not the app's.

### 4.2 Colours (hex)

Every row marked `mac-native` is measured off the running app (§1.1) and names its capture; a superseded reading is kept as what it was and where it was read from. Full verdicts: [VERDICTS.md § 4.2](mac-native/VERDICTS.md).

| Role | Light | Dark | Basis |
|---|---|---|---|
| Editor background | **#f7f7f7** | **#1a1a1a** | M `12-light` / `12-dark`. *Light was read as #f9f9f9 from the App Store, MS Store and stats.webp; the ia.net landing pages' #f7f7f7 is the app's value* |
| Body text | **#191919** | **#cccccc** | M `01-light` / `01-dark`. *Light was read as #1c1c1c from the App Store PNG; the syntax PNG's #191919 is the app's value. Dark confirmed exactly* |
| Heading text | #191919 | #cccccc — identical to body | M `12-*`, `14-gutters` |
| Dimmed (Focus Mode / AI-author / strike-through) | **#c6c4c2** | **#707070** | M `13-light-sentence` / `13-dark-sentence`. *Light was read as #cccccc from App Store 06; the landing webp's ≈#c6c4c3 was within 1 of the app's value. Dark confirmed exactly* |
| Number of dim tiers | **one per theme** — Sentence and Paragraph share it | **one per theme** | M `13-*`. *The tiers were assumed plural* |
| Caret / accent | **#00bfff** | **#00bfff** — the same value, not a per-theme accent | M `01-light` / `01-dark`. *Was #00b5ff (App Store PNG), #01c4ff (webp) and #00b2ff (Windows) on light, #00b5ff / #00c3ff on dark: four still-readings of one flat colour the app does not name* |
| Selection fill, window active | **#ccedf8** | **#113d52** | M `08-light` / `08-dark`. *Light had no recorded value; dark was read as ≈#003e4c, the caret blue at low alpha over #1a1a1a* |
| Selection fill, window idle | **#dcdcdc** | **#464646** | M `07-light` / `07-dark`. *A neutral grey with no blue in it — not the "paler accent" ADR 0014 assumed. Only the colour moves; the band's geometry is identical* |
| Selection end handles / bars | **none** | **none** | M `07`–`10`, both themes: no accent pixel at all in any frame holding a selection. *"handles #00b5ff" was read from `appstore-mac-04`, whose 44 px accent runs are round touch grab-handles* |
| Web preview paper (dark) | — | **#101010** | M `16-preview-full` — a distinctly darker ground than the editor's #1a1a1a, and the two sit side by side in a split |
| Wikilink brackets / link | — / — | #707070 / #00bfff underlined | M. *The link was read as #00b5ff from a marketing still and is corrected to the app's measured accent; wikilinks themselves were not covered by the `mac-native` states (VERDICTS 4.2.13)* |
| Content-block chip (light) | fill #ffffff, border #e1e1e1 | — | M, marketing stills only — not covered by the `mac-native` states (VERDICTS 4.2.13) |
| Autocomplete popup (dark) | — | #3a3a3a | M, marketing stills only — not covered (VERDICTS 4.2.13) |
| Library list (light) | bg #fdfdfd, secondary text #999999, selection bar #00c3ff | — | M, marketing stills only — not covered (VERDICTS 4.2.13); the accent reading is a still's, and the app's accent is #00bfff |
| Syntax: verbs / adjectives / adverbs / conjunctions / nouns | #3476b9 / #a66500 / #b24fa2 / #3f831e / ≈#ca471a (U) | #799fc2 / #c1934e / ≈#ba8eb2 / — / — | M (light exact from lossless PNG) — not covered by the `mac-native` states, which had Syntax off throughout (VERDICTS 4.2.14) |
| Authorship author colours (dark) | — | #faa25d, #fb8b65, #fc686f, #d177ce, #de71b2, #00bfff family | M, marketing stills only — not covered by the `mac-native` states, which hid Authors (VERDICTS 4.2.15) |

Third-party approximations (for cross-checking only, NOT iA's): acheronfail/ia-writer-sublime dark scheme uses background #1d1f20, text #c5c9c6, dimmed #707070, accent rgba(21,189,236) (≈#15bdec) — copied to templates/third-party-acheronfail-ia-writer-dark.sublime-color-scheme.json.

### 4.3 Preview template (from iainc/iA-Writer-Templates)

* `Example.iatemplate/Contents/Resources/style.css` in the public repo is **empty (0 bytes)** — it is a scaffold. The built-in "Modern / Classic / Academic" template CSS ships only inside the app bundle and is not public (GAP): the `mac-native` captures render the template, they do not open it (VERDICTS 4.3.1).
* `GitHub.iatemplate/Contents/Resources/github.css` (real values): `@media screen { .markdown-body { margin: 0 auto; max-width: 830px; } }`; horizontal padding 15 px (≤420 px), 25 px (420–500), 35 px (500–550), 45 px (≥550), 75 px for print; uses GitHub's light/dark markdown CSS.
* Info.plist example: `IATemplateHeaderHeight` / `IATemplateFooterHeight` = 90 (CSS points, ≤400).
* Preview classes: `night-mode`, `ios`, `mac`, `content-size-xs … xxxl`, `content-size-accessibility-m … xxxl` (iOS Dynamic Type). Reload template: ⇧⌘R; Web Inspector: `defaults write pro.writer.mac WebKitDeveloperExtras -bool true`. Not covered by the `mac-native` captures, which did not open the DOM (VERDICTS 4.3.3).

MEASURED against the running app's Preview, three frames taken after the fifteen states. These three are shot with macOS's drop shadow around the window, so the window content sits at x 111 … 3136 and its centre is **1624.0** — not §4.1's 1512.0, which is the centre of the 3024 px frame the other fifteen states were captured to.

| Item | Value | Basis |
|---|---|---|
| Web preview paper (dark) | **#101010** — a distinctly darker ground than the editor's #1a1a1a | M `16-preview-full` |
| Rendered heading alignment | **centred**: ink centre 1627.0 against a window centre of 1624.0, where the editor left-aligns a heading on the body column | M `16-preview-full` |
| Markdown markers in the rendered preview | **gone entirely** — no `#`, no `**`, and a `-` list becomes real bullets, so the editor's hanging-marker gutter has no counterpart there | M `16-preview-full`, `16-preview-split` |
| Split view | divides the window **evenly** (divider x 1621 = 49.9 %), and the two panes carry **different papers** — the editor's #1a1a1a beside the preview's #101010 | M `16-preview-split` |
| PDF preview against Dark Mode | the **whole window goes light** with Dark Mode still on — toolbar #ffffff, surround #f7f7f7 — where the Web preview stays dark | M `16-preview-pdf-full` |
| PDF pagination | the page is centred and 1323 px wide (x 962 … 2284, centre 1623.0), with a page number at its foot; its height is clipped by the viewport and cannot be read | M `16-preview-pdf-full` |

---

## 5. Performance

### 5.1 What iA has published (no latency numbers exist)

iA publishes **no keystroke-latency, frame-time or startup-time measurements**. Every performance statement found is qualitative:

* ia.net/writer (2026): "Each version is made for its platform: fast, focused, and private."
* Windows 2.0 announcement (2024-12-11, https://ia.net/topics/ia-writer-for-windows-2-0): "Our main goal was to improve iA Writer's general responsiveness. It now boots in a whizz, and it is snappy as jazz when you type." / "we split the code base into two. The first part is a minimal, fast-loading core module that starts immediately, followed by a group of secondary components that load in the background." / "For time-consuming tasks, we split processes into two stages: The first sends immediate feedback, while the second handles heavier workloads. We also rewrote several subsystems." / "Above everything else, we care about how Writer feels when you type. Speed, snap, and response make writing feel seamless." / footnote: "Features like Spell Check, Syntax Control, and Style Check now begin analyzing text a few seconds after the application launches." (The post embeds a startup-time comparison video; no number is stated.)
* "Simple, Slick & Smooth" (2025-02-17, https://ia.net/topics/ia-writer-for-windows-2-0-released-into-the-wild): "1. Faster Startup Time and Responsiveness — … Writer for Windows 2.0 is now faster and more supple than… an oiled eel."
* Windows release notes (https://ia.net/writer/support/help/version-history/release-notes-windows): "Modular internal architecture for enhanced performance." / "Improved startup time" / "Scrollbar is now hidden in Typewriter Mode".
* Mac version history (https://ia.net/writer/support/help/version-history): 5.6 "Markdown files open up to 350 times faster / More responsive when editing large files"; Style Check: "Fast: optimized to work in real-time"; "Improved text editing performance"; Quick Open: "Blazing-fast, it searches through your Library"; misc "Performance optimizations and improvements".
* Third-party observation (acheronfail/ia-writer-sublime README, 2018-era): "Over the years, there have been several updates to iA Writer which cause it to hang, pause, or just eat CPU" — anecdotal, UNVERIFIED.

### 5.2 Bar to beat — third-party native-editor latency measurements

**Tristan Hume, "Measuring keyboard-to-photon latency with a light sensor" (2020-05-20, https://thume.ca/2020/05/20/making-a-latency-tester/)** — Teensy LC at 1000 Hz USB polling + photodiode, macOS, Dell P2415Q 60 Hz. Full keyboard-to-photon, character insertion (`lat i=` mean ± sd):
* Sublime Text: `lat i= 32.5 +/- 4.0` ms
* TextEdit: `lat i= 33.4 +/- 1.4` ms
* Atom: `lat i= 45.6 +/- 7.0` ms
* VS Code: `lat i= 47.6 +/- 7.0` ms
* Apple Terminal 35.8 ± 7.0; kitty 36.1 ± 5.6; iTerm2 (GPU) 53.1 ± 6.6 ms
* Note on displays: "The HP Z27 is around 30ms slower than the Dell P2415Q!" — a large part of end-to-end latency is the monitor.

**Pavel Fatin, "Typing with Pleasure" (2015-12, https://pavelfatin.com/typing-with-pleasure/)** — Typometer (screen-capture based, excludes keyboard and display hardware). Core i5-3427U, Intel HD 4000, 1920×1080 @ 59.94 Hz. Windows, plain text file, ms (min / avg / max / SD):
* GVim 0.2 / 0.9 / 1.2 / 0.2 · IDEA zero-latency 0.1 / 2.9 / 21.2 / 2.7 · Notepad++ 0.1 / 4.3 / 5.9 / 0.8 · Emacs 4.2 / 5.3 / 19.2 / 1.1 · **Sublime Text 6.2 / 8.2 / 35.2 / 2.0** · Eclipse 0.1 / 10.1 / 20.8 / 1.6 · Netbeans 7.3 / 11.8 / 31.6 / 3.9 · IDEA default 0.1 / 24.7 / 83.7 / 12.0 · Atom 29.2 / 49.4 / 85.5 / 7.2
* Linux (XML file): IDEA zero-latency 1.7 avg · GVim 4.5 · Gedit 12.4 · Emacs 20.3 · Sublime 23.1 · Netbeans 24.5 · Atom 32.5 · Eclipse 46.5 · IDEA default 69.2
* Hardware budget he cites: keyboard 8–22 ms (avg 14: matrix scan ~0.5, debounce 7–12, USB poll 0–8, transmission 1); monitor ≈ 12 ms avg (refresh 0–17, pixel response ~4).
* Perception: "one does not necessarily need to perceive latency consciously to be affected by it"; motor control is affected by latencies "as small as 20 ms"; "Use a responsive editor (makes the most difference)".

**Dan Luu, "Computer latency: 1977–2017" (https://danluu.com/input-lag/)**: "For very simple tasks, people can perceive latencies down to 2 ms or less." / "increasing latency is not only noticeable to users, it causes users to execute simple tasks less accurately." End-to-end keypress-to-screen: Apple 2e 30 ms, TI 99/4A 40 ms, modern machines 50–200 ms (custom Haswell-E @165 Hz: 50 ms).
**Dan Luu, "Terminal latency" (https://danluu.com/term-latency/)**, mid-2014 13" MacBook Pro 2.6 GHz, 2560×1600 (idle / loaded 50th %ile, ms): terminal.app 6 / 13; emacs-eshell 5 / 13; emacs-term 13 / 30; st 25 / 27; alacritty 31 / 28; hyper 32 / 31; iterm2 44 / 45. (Measured differently from Hume — app-internal, not photon.)

### 5.3 Suggested target derived from the above

* App-internal keystroke → committed frame: ≤ 5 ms average, ≤ 16 ms worst case (Sublime/Notepad++/GVim class in Fatin's data), never dropping a frame while typing.
* Keyboard-to-photon on a 60 Hz Mac display: ≈ 30–35 ms (Sublime/TextEdit class in Hume's data); anything above ~45 ms puts you in the Electron bracket.
* Cold start: no iA number exists; iA's own framing ("boots in a whizz", core module first, analysis "a few seconds after launch") suggests: first paint with editable text before any spell/syntax/library work.

---

## 6. Gaps / UNVERIFIED items

1. Official hex palette — not published. The editor's own paper, ink, dimmed tier, caret and selection are now measured off the running app (§1.1, §4.2); what is still read only off marketing stills is the **syntax palette**, the **authorship author colours**, and the **wikilink and content-block chip chrome** — the `mac-native` states had Syntax off, Authors hidden, and shot neither a wikilink nor a chip.
2. Exact noun-red and dark-theme syntax colours; conjunction green on dark not captured.
3. Whether the "liquid" typography also varies the face's **weight and width** with size — the 14-step ladder measures advance, pitch, band and caret only. The line-spacing half of the curve is measured (§4.1).
4. **Duo's** cell behaviour in the running app — every `mac-native` state was shot in **Mono** so that a cell pitch could be fitted; Duo's 0.6 em comes from the font tables, not from the app.
5. Built-in preview template CSS (Modern/Classic/Academic) — only inside the app bundle; the public repo's Example style.css is empty. The captures render the template, they do not open it.
6. Whether the italic proportional-looking body in `write-ia-writer.webp` and `Writer-Mac-Syntax-Highlight-Display.webp` is Quattro Italic — inferred from glyph widths only.
7. No iA latency/startup numbers exist; the bar is set from third-party measurements of other editors.
8. The "blue wider caret" quote could not be traced to a live ia.net page.
