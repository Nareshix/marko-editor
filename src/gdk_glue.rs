use gtk::prelude::StyleContextExt;

pub trait Serialize<T> {
    fn deserialize(data: &str) -> Option<T>;
    fn serialize(&self) -> String;
}

impl Serialize<gdk::Rectangle> for gdk::Rectangle {
    fn deserialize(data: &str) -> Option<gdk::Rectangle> {
        let mut a = data.split('_');
        if a.next()?.ne("rect") {
            return None;
        }
        Some(gdk::Rectangle::new(
            a.next()?.parse::<i32>().ok()?,
            a.next()?.parse::<i32>().ok()?,
            a.next()?.parse::<i32>().ok()?,
            a.next()?.parse::<i32>().ok()?,
        ))
    }

    fn serialize(&self) -> String {
        format!(
            "rect_{}_{}_{}_{}",
            self.x().to_string(),
            self.y().to_string(),
            self.width().to_string(),
            self.height().to_string()
        )
    }
}

pub trait GetColor {
    fn get_color(&self, is_background: bool, flags: gtk::StateFlags) -> Option<gdk::RGBA>;
}

impl GetColor for gtk::StyleContext {
    fn get_color(&self, is_background: bool, flags: gtk::StateFlags) -> Option<gdk::RGBA> {
        if is_background {
            None
        } else {
            let saved = self.state();
            self.set_state(flags);
            let color = self.color();
            self.set_state(saved);
            Some(color)
        }
    }
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let v = max;
    let s = if max == 0.0 { 0.0 } else { delta / max };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        let mut h = (g - b) / delta;
        if h < 0.0 {
            h += 6.0;
        }
        h / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (v, v, v);
    }
    let h6 = h * 6.0;
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

pub trait ColorCreator {
    fn brighter(&self, factor: f32) -> gdk::RGBA;
}

impl ColorCreator for gdk::RGBA {
    fn brighter(&self, factor: f32) -> gdk::RGBA {
        let (h, mut s, mut v) = rgb_to_hsv(self.red(), self.green(), self.blue());
        v *= factor / 100f32;
        if v > 1f32 {
            s -= v - 1f32;
            if s < 0f32 {
                s = 0f32;
            }
            v = 1f32;
        }

        let (red, green, blue) = hsv_to_rgb(h, s, v);
        gdk::RGBA::new(red, green, blue, self.alpha())
    }
}