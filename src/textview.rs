use crate::textbufferext::{get_file_name, is_file, TextBufferExt2};
use crate::textbuffermd::{TextBufferMd, NEWLINE};
use crate::texttag::{CharFormat, ParFormat, Tag, TextTagExt2, COLORS};
use crate::texttagmanager::{TextEdit, TextTagManager};
use crate::textviewext::TextViewExt2;
use crate::{builder_get, connect, connect_fwd1};

extern crate html_escape;

use gtk::gio::File;
use gtk::glib;
use gtk::glib::signal::Inhibit;
use gtk::glib::Value;
use gtk::prelude::*;
use gtk::prelude::{Cast, ObjectExt};
use gtk::EventControllerKey;

use regex::Regex;
use std::cell::Cell;
use std::collections::HashMap;

use crate::gdk_glue::{ColorCreator, GetColor};
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use std::thread;

mod keys {
    pub use gdk::keys::constants::*;
}

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
    // ToDo: this client should not be created every time!
    let custom = reqwest::redirect::Policy::custom(|attempt| attempt.follow());
    let client =
        reqwest::blocking::Client::builder().redirect(custom).user_agent("Wget/1.21.1").build()?;
    client.get(url).send()
}

fn fetch_title<F: Fn(&str) + 'static>(url: &str, and_then: F) {
    // See: https://coaxion.net/blog/2019/02/mpsc-channel-api-for-painless-usage-of-threads-with-gtk-in-rust/
    // Create a new sender/receiver pair with default priority
    let (sender, receiver) = gtk::glib::MainContext::channel::<String>(glib::PRIORITY_DEFAULT);

    // Spawn the thread and move the sender in there
    let u = String::from(url);
    thread::spawn(move || {
        if let Ok(res) = blocking_get(u.as_str()) {
            if let Ok(text) = res.text() {
                lazy_static! {
                    // ToDo: this is most likely not helpful with the async setup
                    static ref RE: Regex = Regex::new(r"<title[^>]*>([^<]*)<").unwrap();
                }
                if let Some(caps) = RE.captures(&text) {
                    if let Some(c) = caps.get(1) {
                        let decoded =
                            String::from(html_escape::decode_html_entities(c.as_str().trim()));
                        // Sending fails if the receiver is closed
                        let _ = sender.send(decoded);
                    }
                }
            }
        }
    });

    // Attach the receiver to the default main context (None)
    receiver.attach(None, move |msg| {
        and_then(msg.as_str());
        // Returning false here would close the receiver and have senders fail
        gtk::glib::Continue(true)
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
    // https://python-gtk-3-tutorial.readthedocs.io/en/latest/textview.html
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
            self.search_bar.key_capture_widget().grab_focus();
            self.search_bar.set_key_capture_widget::<gtk::Widget>(None);
        }
    }

    fn on_next_match(&self, backward: bool) {
        let buffer = self.buffer();
        let text = String::from(self.edt_search.text().as_str().trim());
        if text.is_empty() {
            return;
        }

        let mut cursor = buffer.get_insert_iter();
        // selection_bounds retrieves the iterators in order
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

        // move view
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

        // highlight all
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
        Self {
            outline_none: gdk::RGBA::black(),
            outline_h1: gdk::RGBA::black(),
            outline_h2: gdk::RGBA::black(),
            outline_h3: gdk::RGBA::black(),
            outline_h4: gdk::RGBA::black(),
            outline_h5: gdk::RGBA::black(),
            outline_h6: gdk::RGBA::black(),
        }
    }

    pub fn update(&mut self, style_context: &gtk::StyleContext, prefer_dark: bool) {
        self.outline_none = GetColor::get_color(style_context, true, gtk::StateFlags::LINK)
            .unwrap_or_else(gdk::RGBA::white);
        self.outline_h1 = GetColor::get_color(style_context, true, gtk::StateFlags::SELECTED)
            .unwrap_or_else(gdk::RGBA::blue);

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
    image_anchors: Rc<RefCell<Vec<(gtk::TextChildAnchor, String)>>>,
}
impl TextView {
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
            image_anchors: Rc::new(RefCell::new(Vec::new())),
        };
        this.top_level.add_controller(&this.get_key_press_handler_background());
        this.textview.add_controller(&this.get_key_press_handler());
        this.textview.add_controller(&this.get_mouse_release_handler());
        this.textview.add_controller(&this.get_drag_handler());
        this.textview.add_controller(&this.get_drop_handler());
        this.textview.set_buffer(Some(&this.buffer));

        this.textview.connect_query_tooltip({
            |t, x, y, keyboard_mode, tooltip| t.tooltip(x, y, keyboard_mode, tooltip)
        });
        this.textview.connect_move_cursor({
            let tags = this.tags.clone();
            move |textview, _, _, _| tags.move_cursor(textview)
        });

        this.link_edit.set_accept_link_cb(connect_fwd1!(this.accept_link()));

        this.buffer
            .connect_local("insert-text", true, connect_fwd1!(this.buffer_do_insert_text()))
            .unwrap();

        this.buffer.connect_changed({
            let this2 = this.clone();
            move |_| {
                this2.reapply_list_tags();
            }
        });
        this.update_colors(false);

        this
    }

    fn reapply_list_tags(&self) {
        if *self.is_renumbering.borrow() {
            return;
        }

        let buffer = &self.buffer;
        let line_count = buffer.line_count();
        let mut ol_counter: u64 = 0;
        let mut in_ol = false;

        for line in 0..line_count {
            let line_iter = match buffer.iter_at_line(line) {
                Some(i) => i,
                None => continue,
            };
            let mut line_end = line_iter.clone();
            if !line_end.ends_line() {
                line_end.forward_to_line_end();
            }

            let line_text = buffer.text(&line_iter, &line_end, false);
            let (indent, item_text) = split_indent(line_text.as_str());

            let is_ul = item_text.starts_with("• ");
            // instead of: let num = &item_text[..p];
            let is_ol = !is_ul && {
                let dot_pos = item_text.find(". ");
                dot_pos
                    .map(|p| {
                        let num: String =
                            item_text.chars().take(item_text[..p].chars().count()).collect();
                        !num.is_empty() && num.chars().all(|c| c.is_ascii_digit())
                    })
                    .unwrap_or(false)
            };
            if is_ul {
                in_ol = false;
                ol_counter = 0;

                let fresh_start = match buffer.iter_at_line(line) {
                    Some(i) => i,
                    None => continue,
                };
                let mut fresh_end = fresh_start.clone();
                if !fresh_end.ends_line() {
                    fresh_end.forward_to_line_end();
                }

                let line_tag = buffer.tag_table().lookup(Tag::LIST_UL).unwrap();
                if !fresh_start.has_tag(&line_tag) {
                    buffer.apply_tag(&line_tag, &fresh_start, &fresh_end);
                }

                let fresh_start2 = match buffer.iter_at_line(line) {
                    Some(i) => i,
                    None => continue,
                };
                let mut prefix_end = fresh_start2.clone();
                prefix_end.forward_chars(2);

                let prefix_tag = buffer.tag_table().lookup(Tag::LIST_UL_PREFIX).unwrap();
                if !fresh_start2.has_tag(&prefix_tag) {
                    buffer.apply_tag(&prefix_tag, &fresh_start2, &prefix_end);
                }
            } else if is_ol {
                let dot_pos = item_text.find(". ").unwrap();
                // instead of: item_text[..dot_pos].parse().unwrap_or(0)
                let current_num: u64 = {
                    let char_count = item_text[..dot_pos].chars().count();
                    let num: String = item_text.chars().take(char_count).collect();
                    num.parse().unwrap_or(0)
                };
                if !in_ol {
                    ol_counter = 1;
                    in_ol = true;
                } else {
                    ol_counter += 1;
                }

                if current_num != ol_counter {
                    let old_prefix = format!("{}{}. ", indent, current_num);
                    let correct_prefix = format!("{}{}. ", indent, ol_counter);

                    let mut prefix_end = match buffer.iter_at_line(line) {
                        Some(i) => i,
                        None => continue,
                    };
                    prefix_end.forward_chars(old_prefix.chars().count() as i32);
                    let mut ls = match buffer.iter_at_line(line) {
                        Some(i) => i,
                        None => continue,
                    };

                    *self.is_renumbering.borrow_mut() = true;
                    buffer.delete(&mut ls, &mut prefix_end);
                    let mut ins = buffer.iter_at_line(line).unwrap();
                    buffer.insert(&mut ins, &correct_prefix);
                    *self.is_renumbering.borrow_mut() = false;
                }

                let fresh_start = match buffer.iter_at_line(line) {
                    Some(i) => i,
                    None => continue,
                };
                let mut fresh_end = fresh_start.clone();
                if !fresh_end.ends_line() {
                    fresh_end.forward_to_line_end();
                }

                let line_tag = buffer.tag_table().lookup(Tag::LIST_OL).unwrap();
                if !fresh_start.has_tag(&line_tag) {
                    buffer.apply_tag(&line_tag, &fresh_start, &fresh_end);
                }

                let fresh_start2 = match buffer.iter_at_line(line) {
                    Some(i) => i,
                    None => continue,
                };

                let current_text = {
                    let mut end = fresh_start2.clone();
                    if !end.ends_line() {
                        end.forward_to_line_end();
                    }
                    buffer.text(&fresh_start2, &end, false)
                };
                let (_, refreshed_item) = split_indent(current_text.as_str());

                let fresh_start3 = match buffer.iter_at_line(line) {
                    Some(i) => i,
                    None => continue,
                };
                let mut prefix_end2 = fresh_start3.clone();
                let indent_chars = indent.chars().count();
                let prefix_chars = refreshed_item
                    .find(". ")
                    .map(|p| refreshed_item[..p].chars().count() + 2)
                    .unwrap_or(2);
                prefix_end2.forward_chars((indent_chars + prefix_chars) as i32);
                let prefix_tag = buffer.tag_table().lookup(Tag::LIST_OL_PREFIX).unwrap();
                if !fresh_start3.has_tag(&prefix_tag) {
                    buffer.apply_tag(&prefix_tag, &fresh_start3, &prefix_end2);
                }
            } else {
                in_ol = false;
                ol_counter = 0;

                for tag_name in
                    &[Tag::LIST_UL, Tag::LIST_OL, Tag::LIST_UL_PREFIX, Tag::LIST_OL_PREFIX]
                {
                    if let Some(tag) = buffer.tag_table().lookup(tag_name) {
                        if line_iter.has_tag(&tag) {
                            buffer.remove_tag(&tag, &line_iter, &line_end);
                        }
                    }
                }
            }
        }
    }
    fn try_auto_heading(&self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let buffer = &self.buffer;
        let cursor = buffer.get_insert_iter();
        let mut line_start = cursor.clone();
        line_start.set_line_offset(0);
        let prefix = buffer.text(&line_start, &cursor, false);
        let prefix = prefix.as_str();
        let level = prefix.chars().take_while(|c| *c == '#').count();
        if level == 0 || level > 6 || prefix.len() != level {
            return false;
        }
        let format = match level {
            1 => ParFormat::H1,
            2 => ParFormat::H2,
            3 => ParFormat::H3,
            4 => ParFormat::H4,
            5 => ParFormat::H5,
            6 => ParFormat::H6,
            _ => return false,
        };
        let tag_name = Tag::from_par_format(&format);
        let tag = buffer.tag_table().lookup(tag_name).unwrap();
        buffer.begin_user_action();
        let mut ls = line_start.clone();
        let mut cur = cursor.clone();
        buffer.delete(&mut ls, &mut cur);
        let mut pos = buffer.get_insert_iter();
        buffer.insert(&mut pos, " ");
        let tag_end = buffer.get_insert_iter();
        let mut tag_start = tag_end.clone();
        tag_start.backward_char();
        let mut tline_end = tag_start.clone();
        tline_end.forward_line();
        buffer.apply_tag(&tag, &tag_start, &tline_end);
        buffer.end_user_action();
        true
    }

    fn try_auto_list(&self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let buffer = &self.buffer;
        let cursor = buffer.get_insert_iter();
        let mut line_start = cursor.clone();
        line_start.set_line_offset(0);
        let prefix = buffer.text(&line_start, &cursor, false);
        let prefix = prefix.as_str();

        let is_unordered = prefix == "-" || prefix == "*";
        let is_ordered = {
            let trimmed: String = prefix
                .chars()
                .rev()
                .skip_while(|c| *c == '.')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            !trimmed.is_empty()
                && trimmed.chars().all(|c| c.is_ascii_digit())
                && prefix.ends_with('.')
        };

        if !is_unordered && !is_ordered {
            return false;
        }

        // save what we need before any mutation
        let current_line = cursor.line();
        let insert_text = if is_unordered { String::from("• ") } else { format!("{} ", prefix) };

        buffer.begin_user_action();

        // delete the typed prefix
        let mut ls = buffer.iter_at_line(current_line).unwrap();
        let mut cur = buffer.get_insert_iter();
        buffer.delete(&mut ls, &mut cur);

        // insert the new prefix
        let mut pos = buffer.get_insert_iter();
        let prefix_start_offset = pos.offset();
        buffer.insert(&mut pos, &insert_text);

        // re-fetch everything fresh by line number / offset
        let line_iter = buffer.iter_at_line(current_line).unwrap();
        let mut line_end = line_iter.clone();
        if !line_end.ends_line() {
            line_end.forward_to_line_end();
        }

        // margin tag on whole line
        let line_tag = buffer
            .tag_table()
            .lookup(if is_ordered { Tag::LIST_OL } else { Tag::LIST_UL })
            .unwrap();
        buffer.apply_tag(&line_tag, &line_iter, &line_end);

        // grey color on prefix only - re-fetch fresh iterators by offset
        let prefix_start = buffer.iter_at_offset(prefix_start_offset);
        let prefix_end = buffer.get_insert_iter();
        let prefix_tag = buffer
            .tag_table()
            .lookup(if is_ordered { Tag::LIST_OL_PREFIX } else { Tag::LIST_UL_PREFIX })
            .unwrap();
        buffer.apply_tag(&prefix_tag, &prefix_start, &prefix_end);

        buffer.end_user_action();
        true
    }
    fn try_list_continue(&self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let buffer = &self.buffer;
        let cursor = buffer.get_insert_iter();

        if !cursor.ends_line() {
            return false;
        }

        let mut line_start = cursor.clone();
        line_start.set_line_offset(0);
        let line_text = buffer.text(&line_start, &cursor, false);
        let line = line_text.as_str();

        let (indent, item_text) = split_indent(line);

        let (is_ol, next_prefix) = if item_text.starts_with("• ") {
            (false, Some(format!("{}• ", indent)))
        } else {
            let dot_pos = item_text.find(". ");
            if let Some(pos) = dot_pos {
                let num_str: String =
                    item_text.chars().take(item_text[..pos].chars().count()).collect();
                if !num_str.is_empty() && num_str.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(n) = num_str.parse::<u64>() {
                        (true, Some(format!("{}{}. ", indent, n + 1)))
                    } else {
                        (false, None)
                    }
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            }
        };

        if let Some(prefix) = next_prefix {
            // empty item = exit the list
            let prefix_char_len = prefix.trim_start().chars().count();
            let content: String = item_text.chars().skip(prefix_char_len).collect();
            if content.trim().is_empty() && !item_text.trim().is_empty() {
                buffer.begin_user_action();
                let mut ls = line_start.clone();
                let mut cur = cursor.clone();
                buffer.delete(&mut ls, &mut cur);
                buffer.end_user_action();
                return true;
            }

            // save current line number before mutation
            let current_line = cursor.line();

            buffer.begin_user_action();
            let mut pos = cursor.clone();
            buffer.insert(&mut pos, &format!("\n{}", prefix));

            // new line is current_line + 1 - fetch everything fresh by line number
            let new_line = current_line + 1;
            let new_bol = match buffer.iter_at_line(new_line) {
                Some(i) => i,
                None => {
                    buffer.end_user_action();
                    return true;
                }
            };
            let mut new_eol = new_bol.clone();
            if !new_eol.ends_line() {
                new_eol.forward_to_line_end();
            }

            // margin tag on whole new line
            let line_tag =
                buffer.tag_table().lookup(if is_ol { Tag::LIST_OL } else { Tag::LIST_UL }).unwrap();
            buffer.apply_tag(&line_tag, &new_bol, &new_eol);

            // grey color on prefix only - fresh iterators again
            let new_bol2 = match buffer.iter_at_line(new_line) {
                Some(i) => i,
                None => {
                    buffer.end_user_action();
                    return true;
                }
            };
            let mut prefix_end = new_bol2.clone();
            prefix_end.forward_chars(prefix.chars().count() as i32);
            let prefix_tag = buffer
                .tag_table()
                .lookup(if is_ol { Tag::LIST_OL_PREFIX } else { Tag::LIST_UL_PREFIX })
                .unwrap();
            buffer.apply_tag(&prefix_tag, &new_bol2, &prefix_end);

            buffer.end_user_action();
            self.tags.text_edit(TextEdit::NewLine);
            return true;
        }

        false
    }
    fn paste_image(&self) -> bool {
        let clipboard = self.textview.clipboard();

        let this = self.clone();
        clipboard.read_texture_async(None::<&gtk::gio::Cancellable>, move |result| {
            if let Ok(Some(texture)) = result {
                this.save_and_insert_image(texture);
            }
        });

        false
    }

    fn save_and_insert_image(&self, texture: gdk::Texture) {
        let filename = format!(
            "image_{}.png",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        );
        let path = get_image_store_dir().join(&filename);
        let path_str = path.to_string_lossy().to_string();

        if texture.save_to_png(&path_str) {
            let view_w = self.textview.allocated_width() - (MARGIN * 2);
            let natural_w = texture.width();
            let natural_h = texture.height();
            let (initial_w, initial_h) = if natural_w > view_w {
                let scale = view_w as f64 / natural_w as f64;
                (view_w, (natural_h as f64 * scale) as i32)
            } else {
                (natural_w, natural_h)
            };

            self.image_widgets
                .borrow_mut()
                .insert(path_str.clone(), (texture.clone(), initial_w, initial_h));

            let buffer = &self.buffer;
            buffer.begin_user_action();

            let mut cursor = buffer.get_insert_iter();

            if !cursor.starts_line() {
                buffer.insert(&mut cursor, "\n");
            }

            let pos_start = cursor.offset();
            let anchor = buffer.create_child_anchor(&mut cursor);

            self.image_anchors.borrow_mut().push((anchor.clone(), path_str.clone()));

            let image_widget =
                self.create_resizable_image(&texture, &path_str, initial_w, initial_h);
            self.textview.add_child_at_anchor(&image_widget, &anchor);

            buffer.apply_image_offset(&cursor, &path_str, "", pos_start);

            if cursor.is_end() || !cursor.ends_line() {
                buffer.insert(&mut cursor, "\n");
            }

            cursor.forward_line();
            buffer.place_cursor(&cursor);

            buffer.end_user_action();
        }
    }
    fn create_resizable_image(
        &self,
        texture: &gdk::Texture,
        path: &str,
        initial_width: i32,
        initial_height: i32,
    ) -> gtk::Widget {
        let picture = gtk::Picture::for_paintable(Some(texture));
        picture.set_size_request(initial_width, initial_height);
        picture.set_can_shrink(true);
        picture.set_keep_aspect_ratio(true);
        picture.set_hexpand(false);
        picture.set_vexpand(false);
        picture.set_halign(gtk::Align::Start);
        picture.set_valign(gtk::Align::Start);
        picture.set_can_focus(false);
        picture.set_focusable(false);

        let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        container.set_hexpand(false);
        container.set_vexpand(false);
        container.set_halign(gtk::Align::Start);
        container.set_valign(gtk::Align::Start);
        container.set_can_focus(false);
        container.set_focusable(false);
        container.set_margin_top(6);
        container.set_margin_bottom(6);
        container.set_margin_start(4);
        container.append(&picture);

        container.upcast::<gtk::Widget>()
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

    fn set_editable(&self, editable: bool) {
        // ToDo: all formatting needs to be disabled, too
        *self.is_editable.borrow_mut() = editable;
        self.textview.set_editable(editable);
        if editable {
            self.grab_focus();
        }
    }

    fn is_editable(&self) -> bool {
        *self.is_editable.borrow().deref()
    }

    fn buffer_do_insert_text(&self, values: &[Value]) -> Option<Value> {
        let buffer = &self.buffer;
        let iter = &values[1].get::<gtk::TextIter>().unwrap();
        let _text = values[2].get::<&str>().unwrap();
        let count = values[3].get::<i32>().unwrap();

        let mut start = iter.clone();
        start.backward_chars(count);
        self.tags.for_each_edit_tag(|tag: &gtk::TextTag| buffer.apply_tag(tag, iter, &start));

        let mut line_start = iter.clone();
        line_start.set_line_offset(0);
        for tag in line_start.tags() {
            let name = tag.get_name();
            if matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                buffer.apply_tag(&tag, &start, iter);
                break;
            }
        }

        None
    }
    fn get_key_press_handler(&self) -> EventControllerKey {
        let controller = EventControllerKey::new();
        controller.connect_key_pressed({
            let this = self.clone();
            move |_controller: &EventControllerKey,
                  key: gdk::keys::Key,
                  _code: u32,
                  modifier: gdk::ModifierType| {
                if modifier == gdk::ModifierType::CONTROL_MASK {
                    match key {
                        keys::_0 => this.par_format(None),
                        keys::_1 => this.par_format(Some(ParFormat::H1)),
                        keys::_2 => this.par_format(Some(ParFormat::H2)),
                        keys::_3 => this.par_format(Some(ParFormat::H3)),
                        keys::_4 => this.par_format(Some(ParFormat::H4)),
                        keys::_5 => this.par_format(Some(ParFormat::H5)),
                        keys::_6 => this.par_format(Some(ParFormat::H6)),
                        keys::b => this.char_format(CharFormat::Bold),
                        keys::d => this.char_format(CharFormat::Strike),
                        keys::i => this.char_format(CharFormat::Italic),
                        keys::f => this.open_search(),
                        keys::l => this.edit_link(),
                        keys::n => this.apply_text_clear(),
                        keys::t => this.char_format(CharFormat::Mono),
                        keys::v => {
                            if this.paste_image() {
                                return Inhibit(true);
                            }
                            return Inhibit(false);
                        }
                        keys::y => this.redo(),
                        keys::z => {
                            if (modifier & gdk::ModifierType::SHIFT_MASK).is_empty() {
                                this.undo();
                            } else {
                                this.redo();
                            }
                        }
                        keys::Down => this.text_move(false),
                        keys::Up => this.text_move(true),
                        _ => {
                            println!("Unmapped key {} mod {} code {}.", key, modifier, _code);
                            return Inhibit(false);
                        }
                    }
                    return Inhibit(true);
                } else if modifier == gdk::ModifierType::SHIFT_MASK {
                    match key {
                        keys::Tab | keys::ISO_Left_Tab => this.remove_tab(),
                        _ => return Inhibit(false),
                    }
                    return Inhibit(true);
                }
                if modifier.is_empty() {
                    match key {
                        keys::F1 => this.char_format(CharFormat::Green),
                        keys::F2 => this.char_format(CharFormat::Red),
                        keys::F3 => this.char_format(CharFormat::Yellow),
                        keys::F4 => this.char_format(CharFormat::Blue),
                        keys::Tab | keys::ISO_Left_Tab => this.insert_tab(),
                        keys::KP_Enter | keys::Return => {
                            if this.try_list_continue() {
                                return Inhibit(true);
                            }
                            this.tags.text_edit(TextEdit::NewLine);
                            return Inhibit(false);
                        }
                        keys::space => {
                            if this.try_auto_heading() {
                                return Inhibit(true);
                            }
                            if this.try_auto_list() {
                                return Inhibit(true);
                            }
                            return Inhibit(false);
                        }
                        _ => return Inhibit(false),
                    }
                    return Inhibit(true);
                }
                Inhibit(false)
            }
        });
        controller
    }

    fn get_key_press_handler_background(&self) -> EventControllerKey {
        let controller = EventControllerKey::new();
        controller.connect_key_pressed({
            let this = self.clone();
            move |_controller: &EventControllerKey,
                  key: gdk::keys::Key,
                  _code: u32,
                  _modifier: gdk::ModifierType| {
                // just Enter/Return is swallowed by the entries, but this enables every modifier
                // to make them work
                match key {
                    keys::Escape => {
                        this.link_edit.reject();
                        this.search_bar.hide();
                    }
                    keys::KP_Enter | keys::Return => this.link_edit.accept(),
                    _ => return Inhibit(false),
                }
                Inhibit(true)
            }
        });
        controller
    }

    fn get_mouse_release_handler(&self) -> gtk::GestureClick {
        let gesture = gtk::GestureClick::new();
        gesture.connect_pressed({
            let this = self.clone();
            move |gesture, n_press, x, y| {
                if this.buffer.has_selection()
                    || n_press < 2
                    || gesture.clone().upcast::<gtk::GestureSingle>().button() > 1
                {
                    return;
                }

                if let Some(link) = this.textview.get_link_at_location(x, y) {
                    (this.activate_link_cb.as_ref().borrow().deref())(link.as_str());
                }
            }
        });
        gesture
    }

    fn get_drag_handler(&self) -> gtk::DragSource {
        let drag = gtk::DragSource::new();
        drag.connect_prepare({
            let this = self.clone();
            move |drag_source: &gtk::DragSource, x, y| -> Option<gdk::ContentProvider> {
                if let Some(link) = this.textview.get_link_at_location(x, y) {
                    // a drag leaves a one char selection, this should be deleted
                    let cursor = this.buffer.get_insert_iter();
                    this.buffer.select_range(&cursor, &cursor);
                    let file = File::for_uri(&link);
                    return Some(gdk::ContentProvider::for_value(&file.to_value()));
                }
                drag_source.drag_cancel();
                None
            }
        });
        drag
    }

    fn get_drop_handler(&self) -> gtk::DropTarget {
        let mime_uri: &str = "text/uri-list";
        let mime_moz: &str = "text/x-moz-url";

        let handler = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);
        handler.set_types(&[glib::Type::STRING, gtk::gio::File::static_type()]);

        handler.connect_accept({
            move |_target, drop| {
                if let Some(f) = drop.formats() {
                    return f.contain_mime_type(mime_moz) || f.contain_mime_type(mime_uri);
                }
                false
            }
        });

        handler.connect_drop({
            let this = self.clone();
            move |_drop, value, x, y| {
                if let Ok(link) = value.get::<&str>() {
                    this.drop_link(link, x, y);
                    return true;
                }
                false
            }
        });

        handler
    }

    fn drop_link(&self, link: &str, x: f64, y: f64) {
        let (_, by) =
            self.textview.window_to_buffer_coords(gtk::TextWindowType::Text, x as i32, y as i32);
        let line_start = self.textview.line_at_y(by).0;
        let buffer = self.textview.buffer();
        let mut line_end = line_start.clone();
        if !line_end.ends_line() {
            line_end.forward_to_line_end();
        }
        let text = buffer.text(&line_start, &line_end, false);
        buffer.begin_user_action();
        if !text.is_empty() {
            buffer.insert(&mut line_end, NEWLINE);
        }
        let link_offset = line_end.offset();
        let link = link.trim();
        if is_file(link) {
            buffer.insert(&mut line_end, &get_file_name(link));
        } else {
            buffer.insert(&mut line_end, link.as_ref());
        }
        buffer.apply_link_offset(&line_end, link.as_ref(), "", link_offset);
        buffer.place_cursor(&line_end);
        buffer.end_user_action();
    }

    pub fn par_format(&self, format: Option<ParFormat>) {
        if !self.is_editable() {
            return;
        }

        let mut start = self.buffer.get_insert_iter();
        start.set_line(start.line());
        let mut end = start.clone();
        // ToDo: this might be a problem for empty lines
        end.forward_to_line_end();

        self.buffer.apply_paragraph_format(format, &start, &end);
    }

    // Changing the format of the current selection/the current word/the cursor
    // Decide if the format is applied or cleared:
    // * for a selection the first character shall decide
    // * for a word the format at the cursor shall decide
    // * for the cursor, the tag manager knows the current format
    // The MONO format is used as long as the selection doesn't consist of complete lines
    // ToDo: The implementation is far from complete
    pub fn char_format(&self, format: CharFormat) {
        if !self.is_editable() {
            return;
        }

        let tag_str = Tag::from_char_format(&format);
        let b = &self.buffer;

        let toggle_tag = |start: &gtk::TextIter, end: &gtk::TextIter| {
            // ToDo: handle multiline formatting
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

        // links should be formatted completely
        // ToDo: a possible selection should be considered
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
            // Remove overlapping paragraph tags on the whole paragraph
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

    fn accept_link(&self, link_data: Option<&LinkData>) {
        self.set_editable(true);
        if let Some(data) = link_data {
            let buffer = &self.buffer;

            let mut start = buffer.iter_at_mark(&self.link_start);
            let mut end = buffer.iter_at_mark(&self.link_end);
            let tags = start.tags();
            buffer.delete(&mut start, &mut end);
            buffer.insert(&mut end, &data.text);
            start = buffer.iter_at_mark(&self.link_start);

            let tag = if data.is_image {
                buffer.create_image_tag(&data.link)
            } else {
                buffer.create_link_tag(&data.link)
            };
            buffer.apply_tag(&tag, &start, &end);

            for tag in tags {
                if tag.get_image().is_none() && tag.get_link().is_none() {
                    buffer.apply_tag(&tag, &start, &end);
                }
            }
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
            // select current non-whitespace clock, to capture complete links
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
        self.restore_image_widgets();
    }
    pub fn redo(&self) {
        if !self.is_editable() {
            return;
        }
        self.buffer.redo();
        self.restore_image_widgets();
    }

    fn restore_image_widgets(&self) {
        let buffer = &self.buffer;
        let store = self.image_widgets.borrow();
        if store.is_empty() {
            return;
        }
        let mut available_paths: Vec<String> = store.keys().cloned().collect();
        available_paths.sort();
        drop(store);

        let mut orphan_offsets: Vec<i32> = Vec::new();
        let mut iter = buffer.start_iter();
        loop {
            if iter.char() == '\u{FFFC}' && iter.child_anchor().is_none() {
                orphan_offsets.push(iter.offset());
            }
            if !iter.forward_char() {
                break;
            }
        }

        if orphan_offsets.is_empty() {
            return;
        }

        for (i, &offset) in orphan_offsets.iter().rev().enumerate() {
            let path_index = orphan_offsets.len() - 1 - i;
            if let Some(path) = available_paths.get(path_index) {
                let store = self.image_widgets.borrow();
                if let Some((texture, w, h)) = store.get(path).map(|(t, w, h)| (t.clone(), *w, *h))
                {
                    drop(store);
                    let mut pos = buffer.iter_at_offset(offset);
                    let mut end = pos.clone();
                    end.forward_char();

                    buffer.begin_irreversible_action();
                    buffer.delete(&mut pos, &mut end);
                    let mut ins = buffer.iter_at_offset(offset);
                    let anchor = buffer.create_child_anchor(&mut ins);
                    let widget = self.create_resizable_image(&texture, path, w, h);
                    self.textview.add_child_at_anchor(&widget, &anchor);
                    let mut anchors = self.image_anchors.borrow_mut();
                    if let Some(entry) = anchors.iter_mut().find(|(_, p)| p == path) {
                        entry.0 = anchor;
                    }
                    buffer.end_irreversible_action();
                }
            }
        }
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

    pub fn new_content_markdown(&self, markdown: &str) {
        self.buffer.begin_irreversible_action();
        self.buffer.assign_markdown(markdown, false);
        self.buffer.end_irreversible_action();
        self.buffer.place_cursor(&self.buffer.start_iter());
    }

    fn insert_tab(&self) {
        if !self.is_editable() {
            return;
        }
        let mut cursor = self.buffer.get_insert_iter();
        let remainder = cursor.line_offset() % TAB_WIDTH;
        self.buffer.insert(&mut cursor, &" ".repeat((4 - remainder) as usize));
    }

    fn remove_tab(&self) {
        if !self.is_editable() {
            return;
        }
        let mut cursor = self.buffer.get_insert_iter();
        if !cursor.starts_line() {
            cursor.set_line(cursor.line());
        }
        for _ in 0..TAB_WIDTH {
            // ToDo: maybe other whitespace types should be considered
            if cursor.char() == ' ' {
                let mut end = cursor.clone();
                end.forward_char();
                self.buffer.delete(&mut cursor, &mut end);
            } else {
                break;
            }
        }
    }

    fn text_move(&self, up: bool) {
        if !self.is_editable() {
            return;
        }
        self.buffer.text_move(up);
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
        // let start = Instant::now();
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

        // let end = Instant::now();
        // let dur = end.duration_since(start);
        // println!("Elapsed: {}µs", dur.as_micros());

        model
    }

    pub fn update_colors(&self, prefer_dark: bool) {
        self.colors.borrow_mut().update(&self.textview.style_context(), prefer_dark);
    }
}
