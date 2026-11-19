//! Custom drawing: ring gauges, the drifting background glow, bars, per-core bars, the history graph,
//! theme swatches, and a container that can scale its child (pop-in and press animations).

use crate::config::{self, hex_rgb};
use gtk::cairo;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

pub type Rgb = (f64, f64, f64);

/// CSS cubic-bezier easing.
pub fn bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;
    let sx = |u: f64| ((ax * u + bx) * u + cx) * u;
    let sy = |u: f64| ((ay * u + by) * u + cy) * u;
    // solve x(u) = t by Newton, then bisection
    let mut u = t;
    for _ in 0..8 {
        let x = sx(u) - t;
        let d = (3.0 * ax * u + 2.0 * bx) * u + cx;
        if x.abs() < 1e-6 || d.abs() < 1e-6 {
            break;
        }
        u -= x / d;
    }
    if !(0.0..=1.0).contains(&u) || (sx(u) - t).abs() > 1e-4 {
        let (mut lo, mut hi) = (0.0, 1.0);
        u = t;
        for _ in 0..30 {
            if sx(u) < t {
                lo = u;
            } else {
                hi = u;
            }
            u = (lo + hi) / 2.0;
        }
    }
    sy(u)
}

/// The soft ease used everywhere: cubic-bezier(.22,1,.36,1)
pub fn ease_soft(t: f64) -> f64 {
    bezier(0.22, 1.0, 0.36, 1.0, t)
}
/// The springy ease of the glass look: cubic-bezier(.34,1.56,.64,1)
pub fn ease_spring(t: f64) -> f64 {
    bezier(0.34, 1.56, 0.64, 1.0, t)
}
pub fn ease_in_out(t: f64) -> f64 {
    bezier(0.42, 0.0, 0.58, 1.0, t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    (lerp(a.0, b.0, t), lerp(a.1, b.1, t), lerp(a.2, b.2, t))
}

pub fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
    cr.close_path();
}

/// Text centred at (cx, cy) with Pango markup.
pub fn draw_markup(cr: &cairo::Context, markup: &str, cx: f64, cy: f64, family: &str) {
    cr.new_path();
    let layout = pangocairo::functions::create_layout(cr);
    let mut fd = gtk::pango::FontDescription::new();
    fd.set_family(family);
    layout.set_font_description(Some(&fd));
    layout.set_markup(markup);
    let (_, log) = layout.pixel_extents();
    cr.move_to(cx - log.width() as f64 / 2.0 - log.x() as f64, cy - log.height() as f64 / 2.0 - log.y() as f64);
    pangocairo::functions::show_layout(cr, &layout);
    cr.new_path();
}

pub const FONT_FAMILY: &str = "Adwaita Sans, Inter, Cantarell, Noto Sans, sans-serif";

// ---------- Ring gauge ----------

struct RingState {
    from: f64,
    target: f64,
    start: Option<Instant>,
    /// the number counts up separately (eases 12% of the way each frame)
    num: f64,
    color: Rgb,
    color_from: Rgb,
    glass: bool,
    ticking: bool,
}

#[derive(Clone)]
pub struct Ring {
    pub area: gtk::DrawingArea,
    st: Rc<RefCell<RingState>>,
}

impl Ring {
    pub fn new(size: i32) -> Ring {
        let area = gtk::DrawingArea::new();
        area.set_content_width(size);
        area.set_content_height(size);
        area.set_halign(gtk::Align::Center);
        let st = Rc::new(RefCell::new(RingState { from: 0.0, target: 0.0, start: None, num: 0.0, color: hex_rgb(config::GREEN), color_from: hex_rgb(config::GREEN), glass: false, ticking: false }));
        let s2 = st.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let s = s2.borrow();
            let (p, color) = Self::now(&s);
            draw_ring(cr, w as f64, h as f64, p, color, s.num, s.glass, 10.0, 52.0);
        });
        Ring { area, st }
    }

    fn duration(s: &RingState) -> f64 {
        if s.glass {
            1.4
        } else {
            1.1
        }
    }

    fn now(s: &RingState) -> (f64, Rgb) {
        match s.start {
            Some(t0) => {
                let k = (t0.elapsed().as_secs_f64() / Self::duration(s)).min(1.0);
                let c = (t0.elapsed().as_secs_f64() / 0.6).min(1.0);
                (lerp(s.from, s.target, ease_soft(k)), lerp_rgb(s.color_from, s.color, c))
            }
            None => (s.target, s.color),
        }
    }

    pub fn set_glass(&self, g: bool) {
        self.st.borrow_mut().glass = g;
        self.area.queue_draw();
    }

    pub fn target(&self) -> f64 {
        self.st.borrow().target
    }

    /// New value. animate = false jumps straight there (the panel is closed, nothing to see).
    pub fn set(&self, pct: f64, animate: bool) {
        let color = hex_rgb(config::color_for(pct));
        {
            let mut s = self.st.borrow_mut();
            let (cur, ccur) = Self::now(&s);
            if animate {
                s.from = cur;
                s.color_from = ccur;
                s.start = Some(Instant::now());
            } else {
                s.start = None;
                s.num = pct;
            }
            s.target = pct.clamp(0.0, 100.0);
            s.color = color;
        }
        if animate {
            self.kick();
        } else {
            self.area.queue_draw();
        }
    }

    /// Start from empty again (when the panel opens).
    pub fn replay(&self) {
        {
            let mut s = self.st.borrow_mut();
            s.from = 0.0;
            s.num = 0.0;
            s.color_from = s.color;
            s.start = Some(Instant::now());
        }
        self.kick();
    }

    fn kick(&self) {
        if self.st.borrow().ticking {
            return;
        }
        self.st.borrow_mut().ticking = true;
        let st = self.st.clone();
        self.area.add_tick_callback(move |a, _| {
            let mut s = st.borrow_mut();
            let tgt = s.target;
            s.num += (tgt - s.num) * 0.12;
            let num_done = (tgt - s.num).abs() < 0.05;
            if num_done {
                s.num = tgt;
            }
            let arc_done = s.start.map(|t0| t0.elapsed().as_secs_f64() >= Self::duration(&s)).unwrap_or(true);
            if arc_done {
                s.start = None;
            }
            a.queue_draw();
            if num_done && arc_done {
                s.ticking = false;
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
}

/// A ring in a 120 x 120 box (track radius r, line width lw), scaled to the area.
#[allow(clippy::too_many_arguments)]
pub fn draw_ring(cr: &cairo::Context, w: f64, h: f64, pct: f64, color: Rgb, num: f64, glass: bool, lw: f64, r: f64) {
    let size = w.min(h);
    let k = size / 120.0;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = r * k;
    let lw = lw * k;
    let two_pi = 2.0 * std::f64::consts::PI;
    cr.new_path();
    // glass lens in the middle
    if glass {
        let lr = size * 0.33;
        let g = cairo::RadialGradient::new(cx - lr * 0.32, cy - lr * 0.44, 0.0, cx, cy, lr * 1.2);
        g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.16);
        g.add_color_stop_rgba(0.7, 1.0, 1.0, 1.0, 0.02);
        g.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
        cr.arc(cx, cy, lr, 0.0, two_pi);
        let _ = cr.set_source(&g);
        let _ = cr.fill_preserve();
        // a bright upper rim and a soft shadow at the bottom
        let rim = cairo::LinearGradient::new(0.0, cy - lr, 0.0, cy + lr);
        rim.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.28);
        rim.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.0);
        rim.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.18);
        let _ = cr.set_source(&rim);
        cr.set_line_width(1.2 * k.max(0.6));
        let _ = cr.stroke();
    }
    // track
    cr.set_line_width(lw);
    cr.set_source_rgba(1.0, 1.0, 1.0, if glass { 0.09 } else { 0.12 });
    cr.arc(cx, cy, r, 0.0, two_pi);
    let _ = cr.stroke();
    // arc from 12 o'clock, with a soft glow
    let p = (pct.clamp(0.0, 100.0)) / 100.0;
    if p > 0.0005 {
        let a0 = -std::f64::consts::FRAC_PI_2;
        let a1 = a0 + two_pi * p;
        cr.set_line_cap(cairo::LineCap::Round);
        for (extra, alpha) in [(lw * 1.4, 0.07), (lw * 0.8, 0.12), (lw * 0.35, 0.2)] {
            cr.set_line_width(lw + extra);
            cr.set_source_rgba(color.0, color.1, color.2, alpha);
            cr.arc(cx, cy, r, a0, a1);
            let _ = cr.stroke();
        }
        cr.set_line_width(lw);
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.arc(cx, cy, r, a0, a1);
        let _ = cr.stroke();
    }
    // the value
    let big = (30.0 * size / 150.0).max(8.0);
    let small = (15.0 * size / 150.0).max(5.0);
    let markup = format!(
        "<span font_weight=\"800\" font_size=\"{}pt\" foreground=\"#ffffff\" font_features=\"tnum\">{}</span><span font_weight=\"700\" font_size=\"{}pt\" foreground=\"#ffffffb3\">%</span>",
        (big * 0.75).round(),
        config::js_round(num),
        (small * 0.75).round()
    );
    draw_markup(cr, &markup, cx, cy, FONT_FAMILY);
}

// ---------- Background glow ----------

pub struct BgLook {
    pub bg: Rgb,
    pub g1: Rgb,
    pub g2: Rgb,
    pub acc: Rgb,
    pub glowop: f64,
    pub moving: bool,
    pub glass: bool,
}

#[derive(Clone)]
pub struct Background {
    pub area: gtk::DrawingArea,
    look: Rc<RefCell<BgLook>>,
    t0: Instant,
    ticking: Rc<Cell<bool>>,
    last: Rc<Cell<f64>>,
}

fn saturate(c: Rgb, k: f64) -> Rgb {
    let l = 0.2126 * c.0 + 0.7152 * c.1 + 0.0722 * c.2;
    let f = |x: f64| (l + (x - l) * k).clamp(0.0, 1.0);
    (f(c.0), f(c.1), f(c.2))
}

/// 0..1..0 over a period, eased (CSS "alternate" animation).
fn pingpong(t: f64, period: f64) -> f64 {
    let x = (t / period).rem_euclid(2.0);
    let x = if x > 1.0 { 2.0 - x } else { x };
    ease_in_out(x)
}

impl Default for Background {
    fn default() -> Self {
        Self::new()
    }
}

impl Background {
    pub fn new() -> Background {
        let area = gtk::DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_can_target(false);
        let look = Rc::new(RefCell::new(BgLook { bg: hex_rgb("#0b1220"), g1: (0.07, 0.76, 0.65), g2: (0.42, 0.36, 1.0), acc: hex_rgb(config::GREEN), glowop: 0.5, moving: true, glass: false }));
        let b = Background { area, look, t0: Instant::now(), ticking: Rc::new(Cell::new(false)), last: Rc::new(Cell::new(-1.0)) };
        let l2 = b.look.clone();
        let t0 = b.t0;
        b.area.set_draw_func(move |_, cr, w, h| {
            let l = l2.borrow();
            let t = if l.moving { t0.elapsed().as_secs_f64() } else { 0.0 };
            paint_background(cr, w as f64, h as f64, &l, t);
        });
        b
    }

    pub fn set_look(&self, l: BgLook) {
        let moving = l.moving && l.glowop > 0.0;
        *self.look.borrow_mut() = l;
        self.area.queue_draw();
        if moving && !self.ticking.get() {
            self.ticking.set(true);
            let look = self.look.clone();
            let ticking = self.ticking.clone();
            let last = self.last.clone();
            let t0 = self.t0;
            self.area.add_tick_callback(move |a, _| {
                let l = look.borrow();
                if !(l.moving && l.glowop > 0.0) {
                    ticking.set(false);
                    return glib::ControlFlow::Break;
                }
                // about 30 frames a second is plenty for a slow drift
                let now = t0.elapsed().as_secs_f64();
                if now - last.get() >= 1.0 / 30.0 {
                    last.set(now);
                    a.queue_draw();
                }
                glib::ControlFlow::Continue
            });
        }
    }
}

/// Fill the background colour and the glows. Rendered at a quarter of the size (the glows are soft).
pub fn paint_background(cr: &cairo::Context, w: f64, h: f64, l: &BgLook, t: f64) {
    cr.set_source_rgb(l.bg.0, l.bg.1, l.bg.2);
    let _ = cr.paint();
    if l.glowop <= 0.0 || w < 2.0 || h < 2.0 {
        return;
    }
    let q = 4.0;
    let (sw, sh) = (((w / q).ceil() as i32).max(1), ((h / q).ceil() as i32).max(1));
    let Ok(surf) = cairo::ImageSurface::create(cairo::Format::ARgb32, sw, sh) else { return };
    {
        let Ok(c) = cairo::Context::new(&surf) else { return };
        c.scale(1.0 / q, 1.0 / q);
        let vmax = w.max(h) / 100.0;
        let (size, period) = if l.glass { (110.0 * vmax, 18.0) } else { (95.0 * vmax, 14.0) };
        let sat = |c: Rgb| if l.glass { saturate(c, 1.25) } else { c };
        // b1: top -40vmax, left -35vmax; b2: bottom -45vmax, right -40vmax (half a cycle later)
        let blobs = [
            (-35.0 * vmax, -40.0 * vmax, sat(l.g1), pingpong(t, period)),
            (w + 40.0 * vmax - size, h + 45.0 * vmax - size, sat(l.g2), pingpong(t + period / 2.0, period)),
        ];
        for (x, y, col, p) in blobs {
            let s = 1.0 + 0.2 * p;
            let cx = x + size / 2.0 + 14.0 * vmax * p;
            let cy = y + size / 2.0 + 10.0 * vmax * p;
            radial_blob(&c, cx, cy, size * s * std::f64::consts::FRAC_1_SQRT_2, col, 0.75 * l.glowop, 0.68);
        }
        if l.glass {
            // the third glow in the accent colour
            let size = 75.0 * vmax;
            let p = pingpong(t, 23.0);
            let s = lerp(0.85, 1.15, p);
            let dx = lerp(-12.0, 14.0, p) * vmax;
            let dy = lerp(-8.0, 16.0, p) * vmax;
            let cx = 0.15 * w + size / 2.0 + dx;
            let cy = 0.35 * h + size / 2.0 + dy;
            radial_blob(&c, cx, cy, size * s * std::f64::consts::FRAC_1_SQRT_2, l.acc, 0.45 * l.glowop * 0.6, 0.65);
        }
    }
    cr.save().ok();
    cr.scale(q, q);
    let _ = cr.set_source_surface(&surf, 0.0, 0.0);
    cr.source().set_filter(cairo::Filter::Good);
    let _ = cr.paint();
    cr.restore().ok();
}

fn radial_blob(c: &cairo::Context, cx: f64, cy: f64, r: f64, col: Rgb, alpha: f64, stop: f64) {
    let g = cairo::RadialGradient::new(cx, cy, 0.0, cx, cy, r);
    g.add_color_stop_rgba(0.0, col.0, col.1, col.2, alpha);
    g.add_color_stop_rgba(stop * 0.5, col.0, col.1, col.2, alpha * 0.45);
    g.add_color_stop_rgba(stop, col.0, col.1, col.2, 0.0);
    let _ = c.set_source(&g);
    c.arc(cx, cy, r, 0.0, 2.0 * std::f64::consts::PI);
    let _ = c.fill();
}

// ---------- Bars, cores and the history graph ----------

/// A horizontal bar (fraction 0..1).
pub fn bar(frac: f64, color: &str, height: i32, glass: bool) -> gtk::DrawingArea {
    let a = gtk::DrawingArea::new();
    a.set_content_height(height);
    a.set_hexpand(true);
    a.set_valign(gtk::Align::Center);
    let c = hex_rgb(color);
    a.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        let r = h / 2.0;
        rounded_rect(cr, 0.0, 0.0, w, h, r);
        cr.set_source_rgba(1.0, 1.0, 1.0, if glass { 0.08 } else { 0.1 });
        let _ = cr.fill();
        let fw = (w * frac.clamp(0.0, 1.0)).max(if frac > 0.0 { h } else { 0.0 });
        if fw > 0.0 {
            rounded_rect(cr, 0.0, 0.0, fw, h, r);
            cr.set_source_rgb(c.0, c.1, c.2);
            let _ = cr.fill();
            if glass {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.4);
                cr.rectangle(r, 0.0, (fw - 2.0 * r).max(0.0), 1.0);
                let _ = cr.fill();
            }
        }
    });
    a
}

/// One bar per CPU core.
pub fn cores(values: Vec<f64>) -> gtk::DrawingArea {
    let a = gtk::DrawingArea::new();
    a.set_content_height(46);
    a.set_hexpand(true);
    a.set_draw_func(move |_, cr, w, h| {
        let n = values.len().max(1) as f64;
        let gap = 6.0;
        let bw = ((w as f64 - gap * (n - 1.0)) / n).max(1.0);
        for (i, v) in values.iter().enumerate() {
            let x = i as f64 * (bw + gap);
            rounded_rect(cr, x, 0.0, bw, h as f64, 6.0f64.min(bw / 2.0));
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
            let _ = cr.fill_preserve();
            cr.save().ok();
            cr.clip();
            let fh = h as f64 * v.clamp(0.0, 100.0) / 100.0;
            let c = hex_rgb(config::color_for(*v));
            rounded_rect(cr, x, h as f64 - fh, bw, fh, 6.0f64.min(bw / 2.0));
            cr.set_source_rgb(c.0, c.1, c.2);
            let _ = cr.fill();
            cr.restore().ok();
        }
    });
    a
}

/// The history graph: the line and its fill, scaled to 0..top %.
pub fn graph(values: Vec<f64>, top: f64) -> gtk::DrawingArea {
    let a = gtk::DrawingArea::new();
    a.set_content_height(80);
    a.set_hexpand(true);
    let last = values.last().copied().unwrap_or(0.0);
    let c = hex_rgb(config::color_for(last));
    a.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        // dashed middle line
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        cr.set_line_width(1.0);
        cr.set_dash(&[3.0, 4.0], 0.0);
        cr.move_to(0.0, h / 2.0);
        cr.line_to(w, h / 2.0);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
        let n = values.len();
        if n < 2 {
            return;
        }
        let pt = |i: usize| (i as f64 / (n - 1) as f64 * w, h - values[i].min(top) / top * (h - 4.0) - 2.0);
        cr.move_to(pt(0).0, pt(0).1);
        for i in 1..n {
            cr.line_to(pt(i).0, pt(i).1);
        }
        let path = cr.copy_path().ok();
        cr.line_to(w, h);
        cr.line_to(0.0, h);
        cr.close_path();
        cr.set_source_rgba(c.0, c.1, c.2, 0.16);
        let _ = cr.fill();
        if let Some(p) = path {
            cr.append_path(&p);
        }
        cr.set_source_rgb(c.0, c.1, c.2);
        cr.set_line_width(2.0);
        cr.set_line_join(cairo::LineJoin::Round);
        let _ = cr.stroke();
    });
    a
}

/// A round theme / accent swatch.
pub fn swatch(bg: Rgb, g1: Option<Rgb>, g2: Option<Rgb>) -> gtk::DrawingArea {
    let a = gtk::DrawingArea::new();
    a.set_content_width(40);
    a.set_content_height(40);
    a.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        cr.arc(w / 2.0, h / 2.0, w.min(h) / 2.0, 0.0, 2.0 * std::f64::consts::PI);
        cr.clip();
        cr.set_source_rgb(bg.0, bg.1, bg.2);
        let _ = cr.paint();
        if let Some(c) = g1 {
            radial_blob(cr, w * 0.28, h * 0.25, w * 0.62 * 1.1, c, 1.0, 1.0);
        }
        if let Some(c) = g2 {
            radial_blob(cr, w * 0.75, h * 0.82, w * 0.66 * 1.1, c, 1.0, 1.0);
        }
    });
    a
}

// ---------- A container that scales its child ----------

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Scaler {
        pub scale: Cell<f64>,
        pub dx: Cell<f64>,
        pub dy: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Scaler {
        const NAME: &'static str = "BoardScaler";
        type Type = super::Scaler;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Scaler {
        fn constructed(&self) {
            self.parent_constructed();
            self.scale.set(1.0);
        }
        fn dispose(&self) {
            while let Some(c) = self.obj().first_child() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for Scaler {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let w = self.obj();
            let s = self.scale.get();
            let (cw, ch) = (w.width() as f32 / 2.0, w.height() as f32 / 2.0);
            snapshot.save();
            snapshot.translate(&gtk::graphene::Point::new(cw + self.dx.get() as f32, ch + self.dy.get() as f32));
            snapshot.scale(s as f32, s as f32);
            snapshot.translate(&gtk::graphene::Point::new(-cw, -ch));
            let mut c = w.first_child();
            while let Some(ch) = c {
                w.snapshot_child(&ch, snapshot);
                c = ch.next_sibling();
            }
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct Scaler(ObjectSubclass<imp::Scaler>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Scaler {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Scaler {
        let s: Scaler = glib::Object::new();
        child.set_parent(&s);
        s
    }

    pub fn set_scale(&self, k: f64, dy: f64) {
        self.imp().scale.set(k);
        self.imp().dy.set(dy);
        self.queue_draw();
    }

    /// Slide in sideways from dx (and fade in): the server name when the server changes.
    pub fn slide_in(&self, dx: f64) {
        let t0 = Instant::now();
        self.imp().dx.set(dx);
        self.set_opacity(0.0);
        self.add_tick_callback(move |w, _| {
            let k = (t0.elapsed().as_secs_f64() / 0.35).min(1.0);
            let e = ease_soft(k);
            w.imp().dx.set(dx * (1.0 - e));
            w.set_opacity(e);
            w.queue_draw();
            if k >= 1.0 {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    /// Animate the scale (and a vertical offset) with an easing, then call done.
    pub fn animate(&self, from: (f64, f64), to: (f64, f64), ms: u32, ease: fn(f64) -> f64, done: Option<Box<dyn Fn()>>) {
        let me = self.clone();
        let t0 = Instant::now();
        let dur = ms as f64 / 1000.0;
        me.set_scale(from.0, from.1);
        let done = Rc::new(RefCell::new(done));
        self.add_tick_callback(move |w, _| {
            let k = (t0.elapsed().as_secs_f64() / dur).min(1.0);
            let e = ease(k);
            w.set_scale(lerp(from.0, to.0, e), lerp(from.1, to.1, e));
            if k >= 1.0 {
                if let Some(d) = done.borrow_mut().take() {
                    d();
                }
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }
}

/// Glass look: a press squishes the widget, release springs it back.
pub fn press_squish(scaler: &Scaler, amount: f64, glass: Rc<Cell<bool>>) {
    let g = gtk::GestureClick::new();
    g.set_propagation_phase(gtk::PropagationPhase::Capture);
    let s1 = scaler.clone();
    let gl = glass.clone();
    g.connect_pressed(move |_, _, _, _| {
        if gl.get() {
            s1.animate((s1.imp().scale.get(), 0.0), (amount, 0.0), 120, ease_soft, None);
        }
    });
    let s2 = scaler.clone();
    let release = move || {
        if (s2.imp().scale.get() - 1.0).abs() > 1e-3 {
            s2.animate((s2.imp().scale.get(), 0.0), (1.0, 0.0), 600, ease_spring, None);
        }
    };
    let r1 = release.clone();
    g.connect_released(move |_, _, _, _| r1());
    g.connect_cancel(move |_, _| release());
    g.connect_stopped(|_| {});
    scaler.add_controller(g);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_curves() {
        assert!((bezier(0.42, 0.0, 0.58, 1.0, 0.5) - 0.5).abs() < 1e-3);
        assert_eq!(ease_soft(0.0), 0.0);
        assert!((ease_soft(1.0) - 1.0).abs() < 1e-6);
        assert!(ease_soft(0.3) > 0.6, "fast start, slow end");
        let peak = (0..=100).map(|i| ease_spring(i as f64 / 100.0)).fold(0.0, f64::max);
        assert!(peak > 1.0, "the spring overshoots");
        assert!((pingpong(0.0, 10.0)).abs() < 1e-9);
        assert!((pingpong(10.0, 10.0) - 1.0).abs() < 1e-9);
        assert!((pingpong(20.0, 10.0)).abs() < 1e-9);
    }
}
