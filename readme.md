# Notes
* cursor movement for Up/Down arrows
* mouse click → update cursor_para and cursor_offset

```
User does something
        ↓
InputHandler intercepts it
        ↓
Document Model gets updated
        ↓
Renderer reads Document, writes to TextBuffer
        ↓
GTK draws the screen
```
data flows one way only. we never read back from the buffer, ever.

bullet characters and numbers are never stored in text, the renderer generates them from the paragraph kind at display time so list numbering just works automatically.

anchors like images and checkboxes have a stable UUID so when undo destroys GTKs internal anchor we just look it up and recreate it, no more positional guessing.


use set_size_points not set_scale or you get weird line artifacts at tag boundaries.

undo is two stacks we own completely, GTKs built in undo is disabled.

Document is

```Document
  └── Vec<Paragraph>
        ├── kind: what type of block is this
        ├── text: the raw plain text, no symbols
        └── marks: formatting ranges on that text
```
will improve usoing ropey later


non-md features
multi column markdown? (decide on syntax later)
---column---
- Item 1
- Item 2
- Item 3
---column---
This text will now appear on the right side of the list!
You can add more lists or paragraphs here.
---end-column---


# Penora

Penora is a simple WYSIWYG editor for note taking written in Rust and GTK 4. It uses Markdown as storage format and can read simple Markdown files. However, the main focus of Marko Editor is WYSIWYG note taking and not being a 100% compliant Markdown editor.

Some notes

For linux its a binary. windows unzip and press on the exe. mac idk yet. still in alpha. got bugs and is unstable.
some improvements to add later
- better undo and redo system
- fix the text glitch in list items. Doesnt always happen but not uncommon as well
- rest is do as i use it



---
# Prev owner notes
![Marko Editor in link edit mode screenshot](./doc/marko-editor-screenshot.png?raw=true "Marko Editor in link edit mode")

## Background

Marko Editor is a learning project driven by my personal note taking requirements. Coming from a C++ and Qt background this is my first deeper venture into Rust and GTK. So you should expect some shortcomings in the source code:

* Not (yet) idiomatic in several places.

* Sometimes feature driven with technical dept.

* Incomplete error handling with many ``unwrap``.

### Interesting Rust and GTK Parts

While the source code is not perfect, parts of it might serve as examples for GTK 4 development with Rust:

* Clean state management for UI callbacks with macros. Only one ``connect!`` call per callback similar to Qt.

* Restoring the window position on X11.

* Dynamic menu content depending on runtime data.

* Communication with UI thread from worker thread via channels - see also [MPSC Channel API for painless usage of threads with GTK in Rust](https://coaxion.net/blog/2019/02/mpsc-channel-api-for-painless-usage-of-threads-with-gtk-in-rust/)

* Structuring of the application for re-use and modularity - see also [GTK3 Patterns in Rust: Structure](https://blog.samwhited.com/2019/02/gtk3-patterns-in-rust-structure/).

--- ---- ----- ------- ----- ---- ---

## Extras for Note Taking

* WYSIWYG editing with clean diffable file format (Markdown with [CriticMarkup](http://criticmarkup.com/))

* Colors for special highlights

* Link titles are fetched automatically

* A start page can be defined to access the most important notes right after starting

* Bookmarks to important note documents

* Optional outline for large documents

--- ---- ----- ------- ----- ---- ---

## Development Status

**Alpha stage - incomplete, not ready for production.**

If you want to use it anyway, these are some of the issues to look out for:

* The undo/redo stack currently doesn't know about the formatting.

* The formatting works currently only on existing text and not directly while typing.

* Restoring the window position is not 100% reliable.

### Planned Features

**Additional to the known shortcomings, in no particular order and time frame.**

* Image embedding (not only image reference editing like currently)

    * Snippet tool (screenshot with crop)

    * Might support Latex formulas

    * Might support additional diagramming tools (PlantUML, Mermaid, ...)

* Knowledge maps (document in document, similar to OneNote)

--- ---- ----- ------- ----- ---- ---

## Building

### Linux

* Make sure rust (the latest stable version) and the libs for gtk4 and x11 are installed.

* ``cargo run`` to compile and run the program.

* ``make DESTDIR=package/usr`` creates the contents for a standard installation on Linux.

* **Arch Linux**: ``PKGBUILD`` is supplied.

* **Fedora** (Rawhide): ``dnf install graphene-devel gtk4-devel libX11-devel``

--- ---- ----- ------- ----- ---- ---

## License

Marko Editor is distributed under the terms of the GPL version 3. See [LICENSE](LICENSE).
