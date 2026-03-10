#[cfg(feature = "default")]
use gdk::prelude::{Cast, IsA};
#[cfg(feature = "default")]
use gdk4_x11::{X11Display, X11Surface};
#[cfg(feature = "default")]
use gtk::prelude::{GtkWindowExt, NativeExt, SurfaceExt};
#[cfg(feature = "default")]
use gtk::glib;

pub trait WindowGeometry {
    fn get_window_geometry(&self) -> Option<gdk::Rectangle>;
    fn set_window_geometry(&self, rect: &gdk::Rectangle);
    fn get_window_screen(&self) -> Option<gdk::Rectangle>;
}

#[cfg(feature = "default")]
fn get_xdisplay(xd: &X11Display) -> *mut x11::xlib::Display {
    unsafe {
        gdk4_x11::ffi::gdk_x11_display_get_xdisplay(
            glib::translate::ToGlibPtr::to_glib_none(xd).0,
        ) as *mut x11::xlib::Display
    }
}

#[cfg(feature = "default")]
fn get_xid(xs: &X11Surface) -> x11::xlib::XID {
    unsafe {
        gdk4_x11::ffi::gdk_x11_surface_get_xid(
            glib::translate::ToGlibPtr::to_glib_none(xs).0,
        )
    }
}

#[cfg(feature = "default")]
impl<W: IsA<gtk::Window> + IsA<gtk::Native>> WindowGeometry for W {
    fn get_window_geometry(&self) -> Option<gdk::Rectangle> {
        let surface = self.surface()?;
        let xs = surface.clone().downcast::<X11Surface>().ok()?;
        let xd = surface.display().downcast::<X11Display>().ok()?;
        unsafe {
            let dpy = get_xdisplay(&xd);
            let xid = get_xid(&xs);
            let screen = x11::xlib::XDefaultRootWindow(dpy);
            let mut _child: u64 = 0;
            let mut x: i32 = 0;
            let mut y: i32 = 0;

            x11::xlib::XTranslateCoordinates(
                dpy, xid, screen, 0, 0,
                &mut x, &mut y, &mut _child,
            );
            let (width, height) = self.default_size();
            Some(gdk::Rectangle::new(x, y, width, height))
        }
    }

    fn set_window_geometry(&self, rect: &gdk::Rectangle) {
        fn get<W: IsA<gtk::Window> + IsA<gtk::Native>>(
            window: &W,
        ) -> Option<(X11Surface, X11Display)> {
            let surface = window.surface()?;
            let xs = surface.clone().downcast::<X11Surface>().ok()?;
            let xd = surface.display().downcast::<X11Display>().ok()?;
            Some((xs, xd))
        }
        self.set_default_size(rect.width(), rect.height());
        if let Some((xs, xd)) = get(self) {
            unsafe {
                let mut hints = x11::xlib::XWindowChanges {
                    x: rect.x(),
                    y: rect.y(),
                    width: 0,
                    height: 0,
                    border_width: 0,
                    sibling: 0,
                    stack_mode: 0,
                };
                x11::xlib::XConfigureWindow(
                    get_xdisplay(&xd),
                    get_xid(&xs),
                    3,
                    &mut hints,
                );
            }
        }
    }

    fn get_window_screen(&self) -> Option<gdk::Rectangle> {
        let surface = self.surface()?;
        let xd = surface.display().downcast::<X11Display>().ok()?;
        unsafe {
            let dpy = get_xdisplay(&xd);
            let screen = x11::xlib::XDefaultRootWindow(dpy);
            let mut _root: u64 = 0;
            let mut x: i32 = 0;
            let mut y: i32 = 0;
            let mut w: u32 = 0;
            let mut h: u32 = 0;
            let mut _border: u32 = 0;
            let mut _depth: u32 = 0;
            x11::xlib::XGetGeometry(
                dpy, screen,
                &mut _root, &mut x, &mut y,
                &mut w, &mut h,
                &mut _border, &mut _depth,
            );
            Some(gdk::Rectangle::new(x, y, w as i32, h as i32))
        }
    }
}