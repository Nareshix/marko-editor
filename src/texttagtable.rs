use crate::texttag::Tag;
use gtk::prelude::TextTagExt;

#[derive(Debug)]
pub struct TextTagTable {
    table: gtk::TextTagTable,
}

impl TextTagTable {
    const MD_H1: &'static str = "# ";
    const MD_H2: &'static str = "## ";
    const MD_H3: &'static str = "### ";
    const MD_H4: &'static str = "#### ";
    const MD_H5: &'static str = "##### ";
    const MD_H6: &'static str = "###### ";

    const MD_CODE_START: &'static str = "```\n";
    const MD_CODE_END: &'static str = "\n```";

    const MD_BOLD: &'static str = "**";
    const MD_ITALIC: &'static str = "*";
    const MD_MONO: &'static str = "``";
    const MD_STRIKE: &'static str = "~~";

    const MD_RED: &'static str = "{--";
    const MD_RED_END: &'static str = "--}";
    const MD_GREEN: &'static str = "{++";
    const MD_GREEN_END: &'static str = "++}";
    const MD_BLUE: &'static str = "{>>";
    const MD_BLUE_END: &'static str = "<<}";
    const MD_YELLOW: &'static str = "{==";
    const MD_YELLOW_END: &'static str = "==}";

    pub fn new() -> Self {
        let table = gtk::TextTagTable::new();

        let tag_h1 = TextTagTable::create_tag(Tag::H1, &table);
        tag_h1.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);
        tag_h1.set_size_points(24f64);
        tag_h1.set_pixels_above_lines(8);
        tag_h1.set_pixels_below_lines(4);

        let tag_h2 = TextTagTable::create_tag(Tag::H2, &table);
        tag_h2.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);
        tag_h2.set_size_points(22f64);
        tag_h2.set_pixels_above_lines(7);
        tag_h2.set_pixels_below_lines(4);

        let tag_h3 = TextTagTable::create_tag(Tag::H3, &table);
        tag_h3.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);
        tag_h3.set_size_points(20f64);
        tag_h3.set_pixels_above_lines(6);
        tag_h3.set_pixels_below_lines(4);

        let tag_h4 = TextTagTable::create_tag(Tag::H4, &table);
        tag_h4.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);
        tag_h4.set_size_points(18f64);
        tag_h4.set_pixels_above_lines(5);
        tag_h4.set_pixels_below_lines(4);

        let tag_h5 = TextTagTable::create_tag(Tag::H5, &table);
        tag_h5.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);
        tag_h5.set_size_points(16f64);
        tag_h5.set_pixels_above_lines(4);
        tag_h5.set_pixels_below_lines(4);

        let tag_h6 = TextTagTable::create_tag(Tag::H6, &table);
        tag_h6.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);
        tag_h6.set_size_points(14f64);
        tag_h6.set_pixels_above_lines(4);
        tag_h6.set_pixels_below_lines(4);

        let tag_bold = TextTagTable::create_tag(Tag::BOLD, &table);
        tag_bold.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);

        let tag_italic = TextTagTable::create_tag(Tag::ITALIC, &table);
        tag_italic.set_style(gtk::pango::Style::Italic);

        let tag_mono = TextTagTable::create_tag(Tag::MONO, &table);
        tag_mono.set_family(Some("Monospace"));
        let grey = gdk::RGBA::new(0f32, 0f32, 0.3f32, 0.05f32);
        tag_mono.set_background_rgba(Some(&grey));

        let tag_code = TextTagTable::create_tag(Tag::CODE, &table);
        tag_code.set_family(Some("Monospace"));
        let code_bg = gdk::RGBA::new(0.2f32, 0.2f32, 0.2f32, 1.0f32);
        tag_code.set_paragraph_background_rgba(Some(&code_bg));
        tag_code.set_left_margin(30);
        tag_code.set_right_margin(30);
        tag_code.set_indent(2);

        let tag_strike = TextTagTable::create_tag(Tag::STRIKE, &table);
        tag_strike.set_strikethrough(true);

        let tag_red = TextTagTable::create_tag(Tag::RED, &table);
        let red = gdk::RGBA::new(1f32, 0f32, 0f32, 0.4f32);
        tag_red.set_background_rgba(Some(&red));

        let tag_green = TextTagTable::create_tag(Tag::GREEN, &table);
        let green = gdk::RGBA::new(0f32, 1f32, 0f32, 0.4f32);
        tag_green.set_background_rgba(Some(&green));

        let tag_blue = TextTagTable::create_tag(Tag::BLUE, &table);
        let blue = gdk::RGBA::new(0f32, 0.5f32, 1f32, 0.6f32);
        tag_blue.set_background_rgba(Some(&blue));

        let tag_yellow = TextTagTable::create_tag(Tag::YELLOW, &table);
        let yellow = gdk::RGBA::new(1f32, 1f32, 0f32, 0.6f32);
        tag_yellow.set_background_rgba(Some(&yellow));

        let tag_search = TextTagTable::create_tag(Tag::SEARCH, &table);
        let highlight = gdk::RGBA::new(1f32, 0f32, 1f32, 0.4f32);
        tag_search.set_background_rgba(Some(&highlight));

        let _tag_rule = TextTagTable::create_tag(Tag::RULE, &table);

        let tag_list_ul = TextTagTable::create_tag(Tag::LIST_UL, &table);
        tag_list_ul.set_left_margin(32);
        tag_list_ul.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);

        let tag_list_ol = TextTagTable::create_tag(Tag::LIST_OL, &table);
        tag_list_ol.set_left_margin(32);
        tag_list_ol.set_weight(gtk::pango::ffi::PANGO_WEIGHT_BOLD);

        let tag_list_ul_prefix = TextTagTable::create_tag(Tag::LIST_UL_PREFIX, &table);
        let muted = gdk::RGBA::new(0.5f32, 0.5f32, 0.5f32, 1.0f32);
        tag_list_ul_prefix.set_foreground_rgba(Some(&muted));

        let tag_list_ol_prefix = TextTagTable::create_tag(Tag::LIST_OL_PREFIX, &table);
        tag_list_ol_prefix.set_foreground_rgba(Some(&muted));
        Self { table }
    }

    pub fn create_tag(name: &str, table: &gtk::TextTagTable) -> gtk::TextTag {
        let tag = gtk::TextTag::new(Some(name));
        table.add(&tag);
        tag
    }

    pub fn tag_table(&self) -> &gtk::TextTagTable {
        &self.table
    }

    pub fn get_tag(&self, name: &str) -> Option<gtk::TextTag> {
        self.table.lookup(name)
    }

    pub fn md_start_tag(tag: &str) -> Option<&'static str> {
        match tag {
            Tag::H1 => Some(TextTagTable::MD_H1),
            Tag::H2 => Some(TextTagTable::MD_H2),
            Tag::H3 => Some(TextTagTable::MD_H3),
            Tag::H4 => Some(TextTagTable::MD_H4),
            Tag::H5 => Some(TextTagTable::MD_H5),
            Tag::H6 => Some(TextTagTable::MD_H6),
            Tag::CODE => Some(TextTagTable::MD_CODE_START),
            Tag::BOLD => Some(TextTagTable::MD_BOLD),
            Tag::ITALIC => Some(TextTagTable::MD_ITALIC),
            Tag::MONO => Some(TextTagTable::MD_MONO),
            Tag::STRIKE => Some(TextTagTable::MD_STRIKE),
            Tag::RED => Some(TextTagTable::MD_RED),
            Tag::GREEN => Some(TextTagTable::MD_GREEN),
            Tag::BLUE => Some(TextTagTable::MD_BLUE),
            Tag::YELLOW => Some(TextTagTable::MD_YELLOW),
            _ => None,
        }
    }

    pub(crate) fn md_end_tag(tag: &str) -> Option<&'static str> {
        match tag {
            Tag::H1 => Some(""),
            Tag::H2 => Some(""),
            Tag::H3 => Some(""),
            Tag::H4 => Some(""),
            Tag::H5 => Some(""),
            Tag::H6 => Some(""),
            Tag::CODE => Some(TextTagTable::MD_CODE_END),
            Tag::BOLD => Some(TextTagTable::MD_BOLD),
            Tag::ITALIC => Some(TextTagTable::MD_ITALIC),
            Tag::MONO => Some(TextTagTable::MD_MONO),
            Tag::STRIKE => Some(TextTagTable::MD_STRIKE),
            Tag::RED => Some(TextTagTable::MD_RED_END),
            Tag::GREEN => Some(TextTagTable::MD_GREEN_END),
            Tag::BLUE => Some(TextTagTable::MD_BLUE_END),
            Tag::YELLOW => Some(TextTagTable::MD_YELLOW_END),
            _ => None,
        }
    }
}