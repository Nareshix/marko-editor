use crate::textbufferext::{TextBufferExt2, get_file_name, is_file};
use crate::textbuffermd::{NEWLINE, TextBufferMd};
use crate::texttag::{COLORS, CharFormat, ParFormat, Tag, TextTagExt2};
use crate::texttagmanager::{TextEdit, TextTagManager};
use crate::textviewext::TextViewExt2;
use crate::{builder_get, connect, connect_fwd1};

extern crate html_escape;

use gdk::cairo;
use gtk::EventControllerKey;
use gtk::gio::File;
use gtk::glib;
use gtk::glib::Propagation;
use gtk::glib::Value;
use gtk::prelude::*;
use gtk::prelude::{Cast, ObjectExt};

use regex::Regex;
use std::collections::HashMap;

use crate::gdk_glue::{ColorCreator, GetColor};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

const MARGIN: i32 = 10;
const TAB_WIDTH: i32 = 4;

pub struct LinkData {
    text: String,
    link: String,
    is_image: bool,
}

type OpenLinkCb = Rc<RefCell<Box<dyn Fn(&str)>>>;
type AcceptLinkCb = Rc<RefCell<Box<dyn Fn(Option<&LinkData>)>>>;

fn split_indent(line: &str) -> (&str, &str) {
    let trimmed = line.trim_start_matches(' ');
    let indent_len = line.len() - trimmed.len();
    (&line[..indent_len], trimmed)
}

fn get_image_store_dir() -> std::path::PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let dir = base.join("marko-editor").join("images");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn blocking_get(url: &str) -> Result<reqwest::blocking::Response, reqwest::Error> {
    let custom = reqwest::redirect::Policy::custom(|attempt| attempt.follow());
    let client =
        reqwest::blocking::Client::builder().redirect(custom).user_agent("Wget/1.21.1").build()?;
    client.get(url).send()
}

fn fetch_title<F: Fn(&str) + 'static>(url: &str, and_then: F) {
    let (sender, receiver) = mpsc::channel::<String>();
    let u = String::from(url);
    thread::spawn(move || {
        if let Ok(res) = blocking_get(u.as_str()) {
            if let Ok(text) = res.text() {
                lazy_static! {
                    static ref RE: Regex = Regex::new(r"<title[^>]*>([^<]*)<").unwrap();
                }
                if let Some(caps) = RE.captures(&text) {
                    if let Some(c) = caps.get(1) {
                        let decoded =
                            String::from(html_escape::decode_html_entities(c.as_str().trim()));
                        let _ = sender.send(decoded);
                    }
                }
            }
        }
    });

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        match receiver.try_recv() {
            Ok(msg) => {
                and_then(msg.as_str());
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

#[derive(Clone)]
struct LinkEdit {
    link_edit_bar: gtk::SearchBar,
    edt_link_name: gtk::Entry,
    edt_link_target: gtk::Entry,
    btn_accept_link: gtk::Button,
    btn_cancel_link: gtk::Button,
    btn_fetch_title: gtk::Button,
    btn_is_image: gtk::ToggleButton,
    accept_link_cb: AcceptLinkCb,
}

impl LinkEdit {
    pub fn new(b: &gtk::Builder) -> Self {
        let this = Self {
            link_edit_bar: builder_get!(b("link_edit_bar")),
            edt_link_name: builder_get!(b("edt_link_name")),
            edt_link_target: builder_get!(b("edt_link_target")),
            btn_accept_link: builder_get!(b("btn_accept_link")),
            btn_cancel_link: builder_get!(b("btn_cancel_link")),
            btn_fetch_title: builder_get!(b("btn_fetch_title")),
            btn_is_image: builder_get!(b("btn_is_image")),
            accept_link_cb: Rc::new(RefCell::new(Box::new(|_| {}))),
        };
        this.btn_accept_link.connect_clicked(connect!(this.accept()));
        this.btn_cancel_link.connect_clicked(connect!(this.reject()));
        this.btn_fetch_title.connect_clicked(connect!(this.fetch_title()));
        this.edt_link_name.connect_activate(connect!(this.accept()));
        this
    }

    pub fn set_accept_link_cb<F: Fn(Option<&LinkData>) + 'static>(&self, accept_link_cb: F) {
        *self.accept_link_cb.borrow_mut() = Box::new(accept_link_cb);
    }

    fn edit_link(&self, link_data: &LinkData) {
        self.edt_link_name.set_text(&link_data.text);
        self.edt_link_target.set_text(&link_data.link);
        self.link_edit_bar.set_search_mode(true);
        self.btn_is_image.set_active(link_data.is_image);

        if link_data.link.is_empty() || link_data.link == link_data.text {
            lazy_static! {
                static ref RE: Regex = Regex::new(r"^\w+://.*").unwrap();
            }
            if RE.is_match(&link_data.text) {
                self.edt_link_target.set_text(&link_data.text);
                self.edt_link_name.grab_focus();
                self.fetch_title();
            } else {
                self.edt_link_target.grab_focus();
            }
        } else {
            self.edt_link_name.grab_focus();
        }
    }

    pub fn accept(&self) {
        self.hide();
        let link_data = LinkData {
            text: self
                .edt_link_name
                .text()
                .as_str()
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" "),
            link: String::from(self.edt_link_target.text().as_str().trim()),
            is_image: self.btn_is_image.is_active(),
        };
        (self.accept_link_cb.borrow())(Some(&link_data));
    }

    pub fn reject(&self) {
        self.hide();
        (self.accept_link_cb.borrow())(None);
    }

    pub fn hide(&self) {
        self.link_edit_bar.set_search_mode(false);
    }

    fn fetch_title(&self) {
        fetch_title(self.edt_link_target.text().as_str(), {
            let s = self.clone();
            move |decoded| {
                s.edt_link_name.set_text(decoded);
                if s.link_edit_bar.is_search_mode() {
                    s.edt_link_name.grab_focus();
                }
            }
        })
    }
}

type AccessViewCb = Rc<Box<dyn Fn() -> gtk::TextView>>;

#[derive(Clone)]
struct SearchBar {
    search_bar: gtk::SearchBar,
    edt_search: gtk::SearchEntry,
    btn_close_search: gtk::Button,
    access_view_cb: AccessViewCb,
}

impl SearchBar {
    pub fn new<F: Fn() -> gtk::TextView + 'static>(b: &gtk::Builder, access_view_cb: F) -> Self {
        let this = Self {
            search_bar: builder_get!(b("search_bar")),
            edt_search: builder_get!(b("edt_search")),
            btn_close_search: builder_get!(b("btn_close_search")),
            access_view_cb: Rc::new(Box::new(access_view_cb)),
        };
        this.search_bar.connect_entry(&this.edt_search);
        this.search_bar.connect_search_mode_enabled_notify(connect!(this.on_enabled()));

        this.edt_search.connect_activate(connect!(this.on_next_match(false)));
        this.edt_search.connect_next_match(connect!(this.on_next_match(false)));
        this.edt_search.connect_previous_match(connect!(this.on_next_match(true)));
        this.edt_search.connect_search_changed(connect!(this.on_search_changed()));

        this.btn_close_search.connect_clicked(connect!(this.hide()));

        this
    }

    pub fn is_open(&self) -> bool {
        self.search_bar.is_search_mode()
    }
    pub fn hide(&self) {
        self.search_bar.set_search_mode(false);
    }
    pub fn open(&self, text_view: &gtk::TextView) {
        self.search_bar.set_search_mode(true);
        self.search_bar.set_key_capture_widget(Some(text_view));
    }

    fn on_enabled(&self) {
        if !self.is_open() {
            self.clear_highlight();
            if let Some(w) = self.search_bar.key_capture_widget() {
                w.grab_focus();
            }
            self.search_bar.set_key_capture_widget(None::<&gtk::Widget>);
        }
    }

    fn on_next_match(&self, backward: bool) {
        let buffer = self.buffer();
        let text = String::from(self.edt_search.text().as_str().trim());
        if text.is_empty() {
            return;
        }

        let mut cursor = buffer.get_insert_iter();
        if let Some((start, end)) = buffer.selection_bounds() {
            if backward {
                cursor = start;
            } else {
                cursor = end;
            }
        }
        let view: gtk::TextView = (self.access_view_cb)();
        let mut wrap_around = true;
        loop {
            if let Some((mut start, end)) = if backward {
                cursor.backward_search(text.as_str(), gtk::TextSearchFlags::CASE_INSENSITIVE, None)
            } else {
                cursor.forward_search(text.as_str(), gtk::TextSearchFlags::CASE_INSENSITIVE, None)
            } {
                buffer.select_range(&start, &end);
                view.scroll_to_iter(&mut start, 0.05, false, 0., 0.);
                return;
            } else if wrap_around {
                if backward {
                    cursor = buffer.end_iter();
                } else {
                    cursor = buffer.start_iter();
                }
                wrap_around = false;
                continue;
            } else {
                break;
            }
        }
    }

    fn on_search_changed(&self) {
        self.clear_highlight();

        let buffer = self.buffer();
        let tag = buffer.tag_table().lookup(Tag::SEARCH).unwrap();
        let text = String::from(self.edt_search.text().as_str().trim());
        if text.is_empty() {
            return;
        }

        let cursor = buffer.get_insert_iter();
        let view: gtk::TextView = (self.access_view_cb)();
        if let Some((mut start, end)) =
            cursor.forward_search(text.as_str(), gtk::TextSearchFlags::CASE_INSENSITIVE, None)
        {
            buffer.select_range(&start, &end);
            view.scroll_to_iter(&mut start, 0.05, false, 0., 0.);
        } else if let Some((mut start, end)) =
            cursor.backward_search(text.as_str(), gtk::TextSearchFlags::CASE_INSENSITIVE, None)
        {
            buffer.select_range(&start, &end);
            view.scroll_to_iter(&mut start, 0.05, false, 0., 0.);
        }

        let mut iter = buffer.start_iter();
        while let Some((start, end)) =
            iter.forward_search(text.as_str(), gtk::TextSearchFlags::CASE_INSENSITIVE, None)
        {
            buffer.apply_tag(&tag, &start, &end);
            iter = end;
        }
    }

    fn clear_highlight(&self) {
        let buffer = self.buffer();
        let (start, end) = buffer.bounds();
        buffer.remove_tag_by_name(Tag::SEARCH, &start, &end);
    }

    fn buffer(&self) -> gtk::TextBuffer {
        (self.access_view_cb)().buffer()
    }
}

pub struct Colors {
    outline_none: gdk::RGBA,
    outline_h1: gdk::RGBA,
    outline_h2: gdk::RGBA,
    outline_h3: gdk::RGBA,
    outline_h4: gdk::RGBA,
    outline_h5: gdk::RGBA,
    outline_h6: gdk::RGBA,
}

impl Colors {
    pub fn new() -> Self {
        let black = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
        Self {
            outline_none: black,
            outline_h1: black,
            outline_h2: black,
            outline_h3: black,
            outline_h4: black,
            outline_h5: black,
            outline_h6: black,
        }
    }

    pub fn update(&mut self, style_context: &gtk::StyleContext, prefer_dark: bool) {
        self.outline_none = GetColor::get_color(style_context, false, gtk::StateFlags::empty())
            .unwrap_or(gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
        self.outline_h1 = GetColor::get_color(style_context, false, gtk::StateFlags::SELECTED)
            .unwrap_or(gdk::RGBA::new(0.1, 0.4, 0.9, 1.0));

        let factor = if prefer_dark { -15. } else { 15. };

        self.outline_h2 = self.outline_h1.brighter(100. + 1. * factor);
        self.outline_h3 = self.outline_h1.brighter(100. + 2. * factor);
        self.outline_h4 = self.outline_h1.brighter(100. + 3. * factor);
        self.outline_h5 = self.outline_h1.brighter(100. + 4. * factor);
        self.outline_h6 = self.outline_h1.brighter(100. + 5. * factor);
    }
}

#[derive(Clone)]
pub struct TextView {
    buffer: gtk::TextBuffer,
    tags: Rc<TextTagManager>,
    textview: gtk::TextView,
    link_edit: Rc<LinkEdit>,
    search_bar: Rc<SearchBar>,
    activate_link_cb: OpenLinkCb,
    top_level: gtk::Widget,
    is_editable: Rc<RefCell<bool>>,
    link_start: gtk::TextMark,
    link_end: gtk::TextMark,
    colors: Rc<RefCell<Colors>>,
    is_renumbering: Rc<RefCell<bool>>,
    image_widgets: Rc<RefCell<HashMap<String, (gdk::Texture, i32, i32)>>>,
    anchor_registry: Rc<RefCell<Vec<AnchorEntry>>>,
    internal_clipboard: Rc<RefCell<Option<AnchorKind>>>,
    autosave_timer: Rc<RefCell<Option<glib::SourceId>>>,
    autosave_cb: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

#[derive(Clone)]
pub enum AnchorKind {
    Image(String),
    Rule,
    Checkbox,
}

#[derive(Clone)]
pub(crate) struct AnchorEntry {
    pub(crate) anchor: gtk::TextChildAnchor,
    pub(crate) kind: AnchorKind,
    last_offset: i32,
}

impl TextView {
    pub fn set_autosave_cb<F: Fn() + 'static>(&self, f: F) {
        *self.autosave_cb.borrow_mut() = Some(Box::new(f));
    }

    pub fn new() -> Self {
        let ui_src = include_str!("textview.ui");
        let b = gtk::Builder::new();
        b.add_from_string(ui_src).expect("Couldn't add from string");

        let tags = Rc::new(TextTagManager::new());
        let buffer = gtk::TextBuffer::new(Some(tags.table()));

        let textview: gtk::TextView = builder_get!(b("textview"));
        textview.set_top_margin(MARGIN);
        textview.set_bottom_margin(MARGIN);
        textview.set_left_margin(MARGIN);
        textview.set_right_margin(MARGIN);
        textview.set_wrap_mode(gtk::WrapMode::Word);
        textview.set_pixels_above_lines(2);
        textview.set_pixels_below_lines(2);
        textview.set_pixels_inside_wrap(1);
        textview.set_has_tooltip(true);

        let link_edit = Rc::new(LinkEdit::new(&b));
        let search_bar = Rc::new(SearchBar::new(&b, {
            let t = textview.clone();
            move || -> gtk::TextView { t.clone() }
        }));

        let b: gtk::Box = builder_get!(b("container"));
        let top_level = b.upcast::<gtk::Widget>();

        let activate_link_cb: OpenLinkCb = Rc::new(RefCell::new(Box::new(|_: &str| {})));

        let link_start = buffer.create_mark(None, &buffer.start_iter(), true);
        let link_end = buffer.create_mark(None, &buffer.start_iter(), false);

        let autosave_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let autosave_cb: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        buffer.connect_changed({
            let timer = autosave_timer.clone();
            let cb = autosave_cb.clone();
            move |_| {
                if let Some(id) = timer.borrow_mut().take() {
                    id.remove();
                }
                let timer2 = timer.clone();
                let cb2 = cb.clone();
                let id = glib::timeout_add_seconds_local(2, move || {
                    timer2.borrow_mut().take();
                    if let Some(f) = cb2.borrow().as_ref() {
                        f();
                    }
                    glib::ControlFlow::Break
                });
                *timer.borrow_mut() = Some(id);
            }
        });

        let this = Self {
            buffer,
            tags,
            textview,
            link_edit,
            search_bar,
            top_level,
            activate_link_cb,
            is_editable: Rc::new(RefCell::from(true)),
            link_start,
            link_end,
            colors: Rc::new(RefCell::new(Colors::new())),
            is_renumbering: Rc::new(RefCell::new(false)),
            image_widgets: Rc::new(RefCell::new(HashMap::new())),
            anchor_registry: Rc::new(RefCell::new(Vec::new())),
            internal_clipboard: Rc::new(RefCell::new(None)),
            autosave_timer,
            autosave_cb,
        };

        this.top_level.add_controller(this.get_key_press_handler_background());
        this.textview.add_controller(this.get_key_press_handler());
        this.textview.add_controller(this.get_mouse_release_handler());
        this.textview.add_controller(this.get_drag_handler());
        this.textview.add_controller(this.get_drop_handler());
        this.textview.set_buffer(Some(&this.buffer));

        this.textview.connect_query_tooltip({
            |t, x, y, keyboard_mode, tooltip| t.tooltip(x, y, keyboard_mode, tooltip)
        });
        this.textview.connect_move_cursor({
            let tags = this.tags.clone();
            move |textview, _, _, _| tags.move_cursor(textview)
        });

        this.link_edit.set_accept_link_cb(connect_fwd1!(this.accept_link()));

        this.buffer.connect_local("insert-text", true, connect_fwd1!(this.buffer_do_insert_text()));

        this.buffer.connect_changed({
            let this2 = this.clone();
            move |_| {
                this2.reapply_list_tags();
                this2.textview.queue_draw();
            }
        });

        this.update_colors(false);

        this
    }

    pub fn get_widget(&self) -> &gtk::Widget {
        &self.top_level
    }

    pub fn modified(&self) -> bool {
        self.buffer.is_modified()
    }

    pub fn set_not_modified(&self) {
        self.buffer.set_modified(false)
    }

    pub fn grab_focus(&self) {
        self.textview.grab_focus();
    }

    pub fn set_activate_link_cb<F: Fn(&str) + 'static>(&self, activate_link_cb: F) {
        *self.activate_link_cb.borrow_mut() = Box::new(activate_link_cb);
    }

    pub fn scroll_to(&self, line: i32) {
        if let Some(mut iter) = self.textview.buffer().iter_at_line(line) {
            self.textview.scroll_to_iter(&mut iter, 0.05, true, 0., 0.1);
        }
    }

    pub fn scroll_to_top_bottom(&self, to_top: bool) {
        let line = if to_top { 0 } else { self.textview.buffer().line_count() - 1 };
        if let Some(mut iter) = self.textview.buffer().iter_at_line(line) {
            self.textview.scroll_to_iter(&mut iter, 0.05, true, 0., 0.1);
        }
    }

    pub fn par_format(&self, format: Option<ParFormat>) {
        if !self.is_editable() {
            return;
        }

        let mut start = self.buffer.get_insert_iter();
        start.set_line(start.line());
        let mut end = start.clone();
        end.forward_to_line_end();

        self.buffer.apply_paragraph_format(format, &start, &end);
    }

    pub fn char_format(&self, format: CharFormat) {
        if !self.is_editable() {
            return;
        }

        let tag_str = Tag::from_char_format(&format);
        let b = &self.buffer;

        let toggle_tag = |start: &gtk::TextIter, end: &gtk::TextIter| {
            let tag = b.tag_table().lookup(tag_str).unwrap();
            b.begin_user_action();
            if start.has_tag(&tag) {
                b.remove_tag(&tag, start, end);
            } else {
                if COLORS.contains(&format) {
                    for c in &COLORS {
                        let tag = b.tag_table().lookup(Tag::from_char_format(c)).unwrap();
                        b.remove_tag(&tag, start, end);
                    }
                }

                b.apply_tag(&tag, start, end);
            }
            b.end_user_action();
        };

        let start = self.buffer.get_insert_iter();
        if let Some((_, tag)) = self.buffer.get_link_at_iter(&start) {
            if let Some((start, end)) = self.buffer.get_current_tag_bounds(&tag) {
                toggle_tag(&start, &end);
                return;
            }
        } else if let Some((_, tag)) = self.buffer.get_image_at_iter(&start) {
            if let Some((start, end)) = self.buffer.get_current_tag_bounds(&tag) {
                toggle_tag(&start, &end);
                return;
            }
        }

        if let Some((start, mut end)) = b.selection_bounds() {
            if end.starts_line() {
                end.backward_char();
            }
            if format == CharFormat::Mono && start.starts_line() && end.ends_line() {
                b.apply_paragraph_format(Some(ParFormat::Code), &start, &end);
            } else {
                toggle_tag(&start, &end);
            }
        } else if let Some((start, end)) = b.get_current_word_bounds() {
            toggle_tag(&start, &end);
        } else {
            self.tags.toggle_tag(tag_str);
        }
    }

    pub fn apply_text_clear(&self) {
        if !self.is_editable() {
            return;
        }
        let clear = |start: &gtk::TextIter, end: &gtk::TextIter| {
            for line in start.line()..end.line() + 1 {
                if let Some(line_iter) = self.buffer.iter_at_line(line) {
                    for tag in line_iter.tags() {
                        if tag.get_par_format().is_some() {
                            let mut line_end = line_iter.clone();
                            line_end.forward_to_line_end();
                            line_end.forward_char();
                            self.buffer.remove_tag(&tag, &line_iter, &line_end);
                        }
                    }
                }
            }

            self.buffer.remove_all_tags(start, end);
        };

        if let Some((start, end)) = self.buffer.selection_bounds() {
            clear(&start, &end);
        } else if let Some((start, end)) = self.buffer.get_current_word_bounds() {
            clear(&start, &end);
        }
    }

    pub fn edit_link(&self) {
        if !self.is_editable() {
            return;
        }

        let mut start = self.buffer.get_insert_iter();
        let mut end = start.clone();
        let mut link = String::new();
        let mut is_image = false;
        if let Some((l, tag)) = self.buffer.get_link_at_iter(&start) {
            link = l;
            if !start.starts_tag(Some(&tag)) {
                start.backward_to_tag_toggle(Some(&tag));
            }
            if !end.ends_tag(Some(&tag)) {
                end.forward_to_tag_toggle(Some(&tag));
            }
        } else if let Some((l, tag)) = self.buffer.get_image_at_iter(&start) {
            link = l;
            if !start.starts_tag(Some(&tag)) {
                start.backward_to_tag_toggle(Some(&tag));
            }
            if !end.ends_tag(Some(&tag)) {
                end.forward_to_tag_toggle(Some(&tag));
            }
            is_image = true;
        } else if let Some((s, e)) = self.buffer.selection_bounds() {
            start = s;
            end = e;
        } else {
            while start.backward_char() {
                if start.char().is_whitespace() {
                    start.forward_char();
                    break;
                }
            }
            if !end.char().is_whitespace() {
                while end.forward_char() {
                    if end.char().is_whitespace() {
                        break;
                    }
                }
            }
        }

        self.buffer.move_mark(&self.link_start, &start);
        self.buffer.move_mark(&self.link_end, &end);
        let text = String::from(self.buffer.text(&start, &end, false).as_str());

        let old_link = LinkData { text, link, is_image };
        self.search_bar.hide();
        self.link_edit.edit_link(&old_link);
        self.set_editable(false);
    }

    pub fn open_search(&self) {
        if self.search_bar.is_open() {
            self.search_bar.hide();
        } else {
            self.link_edit.reject();
            self.search_bar.open(&self.textview);
        }
    }

    pub fn undo(&self) {
        if !self.is_editable() {
            return;
        }
        self.buffer.undo();
        self.restore_anchors();
    }

    pub fn redo(&self) {
        if !self.is_editable() {
            return;
        }
        self.buffer.redo();
        self.restore_anchors();
    }

    pub fn to_markdown(&self) -> String {
        self.buffer.to_markdown()
    }

    pub fn clear(&self) {
        self.buffer.clear();
    }

    pub fn insert_markdown(&self, markdown: &str, clear: bool) {
        self.buffer.begin_user_action();
        if clear {
            self.buffer.clear();
        }
        self.buffer.insert_markdown(&mut self.buffer.get_insert_iter(), markdown);
        self.buffer.end_user_action();
    }

    pub fn resize_images_to_fit(&self) {
        let available = self.textview.allocated_width();
        if available <= 1 {
            return; // still not laid out, skip
        }

        let registry = self.anchor_registry.borrow();
        for entry in registry.iter() {
            if let AnchorKind::Image(path) = &entry.kind {
                if entry.anchor.is_deleted() {
                    continue;
                }
                // Get the texture we already loaded
                let texture_opt = self.image_widgets.borrow().get(path).map(|(t, _, _)| t.clone());

                if let Some(texture) = texture_opt {
                    let (w, h) = Self::calculate_image_display_size(&texture, available);

                    // Update the cached size
                    self.image_widgets.borrow_mut().entry(path.clone()).and_modify(|e| {
                        e.1 = w;
                        e.2 = h;
                    });

                    // Find and resize the actual widget
                    let widgets = entry.anchor.widgets();
                    for widget in widgets {
                        widget.set_size_request(w, h);
                        // The picture is inside a container box
                        let mut child = widget.first_child();
                        while let Some(c) = child {
                            c.set_size_request(w, h);
                            child = c.next_sibling();
                        }
                    }
                }
            }
        }
    }
    pub fn new_content_markdown(&self, markdown: &str) {
        self.buffer.begin_irreversible_action();
        self.buffer.assign_markdown(markdown, false);
        self.buffer.end_irreversible_action();
        self.buffer.place_cursor(&self.buffer.start_iter());
        self.restore_anchors();

        let textview = self.textview.clone();
        let buffer = self.buffer.clone();
        let this = self.clone();
        glib::idle_add_local(move || {
            this.resize_images_to_fit();
            if let Some(mut iter) = buffer.iter_at_line(0) {
                textview.scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.0);
            }
            glib::ControlFlow::Break
        });
    }

    pub fn get_outline_model(&self, max_level: u32) -> gtk::ListStore {
        let colors = self.colors.borrow();

        let model = gtk::ListStore::new(&[
            glib::GString::static_type(),
            glib::Type::I32,
            gdk::RGBA::static_type(),
        ]);

        let mut line_iter = self.buffer.start_iter();
        let mut line = 0;
        loop {
            for tag in &line_iter.toggled_tags(true) {
                if let Some(par_format) = &tag.get_par_format() {
                    if let Some(level) = Tag::header_level(par_format) {
                        if level <= max_level {
                            let mut line_end = line_iter.clone();
                            line_end.forward_to_line_end();
                            model.set(
                                &model.append(),
                                &[
                                    (
                                        0,
                                        &format!(
                                            "{}{}",
                                            "  ".repeat((level - 1) as usize),
                                            self.buffer.text(&line_iter, &line_end, false)
                                        ),
                                    ),
                                    (1, &line),
                                    (
                                        2,
                                        &match level {
                                            1 => colors.outline_h1,
                                            2 => colors.outline_h2,
                                            3 => colors.outline_h3,
                                            4 => colors.outline_h4,
                                            5 => colors.outline_h5,
                                            6 => colors.outline_h6,
                                            _ => colors.outline_none,
                                        },
                                    ),
                                ],
                            );
                        }
                    }
                    break;
                }
            }
            line += 1;
            if !line_iter.forward_line() {
                break;
            }
        }

        model
    }

    pub fn update_colors(&self, prefer_dark: bool) {
        self.colors.borrow_mut().update(&self.textview.style_context(), prefer_dark);
    }
}
