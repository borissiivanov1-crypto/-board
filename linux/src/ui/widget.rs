//! The desktop widget: the iOS home screen widget's three sizes (small, medium, large) as a small
//! borderless window, refreshed every "Refresh every (minutes)". Click a button to open the service,
//! anywhere else for the dashboard on that server; drag to move; right-click for size and refresh.
//! `board --render-widget out.png` draws one to a PNG file (for desktop tools like Conky).

use super::{bg, Ctx};
use crate::config::{self, color_for, format_bytes, hex_rgb, GLOW_LEVELS};
use crate::glances::{self, Stats};
use crate::layout::{self, Item};
use crate::logos::Logos;
use crate::net;
use crate::secrets::Secrets;
use crate::settings::Settings;
use crate::store::Store;
use gtk::cairo;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

pub fn size_of(family: &str) -> (f64, f64) {
    match family {
        "small" => (170.0, 170.0),
        "large" => (364.0, 382.0),
        _ => (364.0, 170.0),
    }
}

pub fn family(s: &str) -> &'static str {
    match s {
        "small" => "small",
        "large" => "large",
        _ => "medium",
    }
}

#[derive(Clone, Debug)]
pub enum Click {
    Url(String),
    Stack(String),
    Dashboard,
}

/// A clickable place on a widget: x, y, width, height, and what a click does.
pub type Hit = (f64, f64, f64, f64, Click);

#[derive(Clone)]
pub struct Cell {
    pub name: String,
    pub icon: String,
    pub logo: Option<Vec<u8>>,
    pub dot: Option<&'static str>,
    pub click: Click,
}

/// Everything a widget draws (gathered off the main thread).
#[derive(Clone)]
pub struct WidgetData {
    pub server: String,
    pub title: String,
    pub configured: bool,
    pub mon: bool,
    pub reach: bool,
    pub stats: Option<Stats>,
    pub cells: Option<Vec<Cell>>,
    pub up_count: usize,
    pub total: usize,
    pub checked: bool,
    pub link: String,
    pub dots: bool,
    pub time: String,
    pub bg: (f64, f64, f64),
    pub g1: (f64, f64, f64),
    pub g2: (f64, f64, f64),
    pub strength: f64,
    pub glass: bool,
}

/// Gathers what the widget shows: the server's reachability, the stats (Monitoring on) and the buttons.
/// Everything runs side by side, like the iOS widget.
pub fn collect(data_path: &Path, want: Option<&str>, fam: &str, secrets: &Secrets, logos: &Logos) -> WidgetData {
    let mut store = Store::open(data_path);
    let id = store.initial_server(want, true);
    store.cur = id.clone();
    let set: Settings = store.settings();
    let d = store.load_data();
    let title = if set.widget_title.is_empty() { store.title() } else { set.widget_title.clone() };
    let t = config::theme(&set.theme);
    let rgb = |c: (u8, u8, u8)| (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0);
    let normal = GLOW_LEVELS.iter().find(|g| g.0 == "normal").unwrap().1;
    let mut out = WidgetData {
        server: id.clone(),
        title,
        configured: !set.server_host().is_empty(),
        mon: set.monitoring,
        reach: false,
        stats: None,
        cells: None,
        up_count: 0,
        total: 0,
        checked: set.widget_status_dots,
        link: set.link_url.clone(),
        dots: set.widget_status_dots,
        time: chrono::Local::now().format("%H:%M").to_string(),
        bg: hex_rgb(t.bg),
        g1: rgb(t.g1),
        g2: rgb(t.g2),
        strength: if t.no_glow { 0.0 } else { config::glow_level(&set.glow) / normal },
        glass: set.style == "glass",
    };
    if !out.configured {
        return out;
    }
    let host = set.server_host();
    let lay = layout::get_layout(&d, &host);
    let small = fam == "small";
    let large = fam == "large";
    let pass = secrets.glances_pass(&id);
    let want_grid = large || !set.monitoring;
    let max_cells = if large || small { 99 } else { 8 };
    let (reach, stats, grid) = std::thread::scope(|sc| {
        let r = sc.spawn(|| net::server_reachable(&set, &lay.all, 4.0));
        let g = sc.spawn(|| {
            if !want_grid {
                return None;
            }
            let list: Vec<Item> = layout::widget_cells(&lay, set.widget_rows).into_iter().take(max_cells).collect();
            // a stack pill needs the status of every button inside it
            let mut to_check = vec![];
            for c in &list {
                match c {
                    Item::Group(g) => to_check.extend(g.members.iter().cloned()),
                    Item::Svc(s) => to_check.push(s.clone()),
                }
            }
            let urls: Vec<String> = list.iter().map(|c| c.logo().to_string()).collect();
            let (status, _) = std::thread::scope(|s2| {
                let st = s2.spawn(|| if set.widget_status_dots { net::check_services(&to_check, (set.ping_timeout as f64).min(3.0)) } else { HashMap::new() });
                let lg = s2.spawn(|| if set.use_logos { logos.ensure(&urls, 4.0, &net::get_bytes) } else { false });
                (st.join().unwrap_or_default(), lg.join().unwrap_or(false))
            });
            Some((list, status, to_check.len()))
        });
        let s = if set.monitoring {
            let api = |p: &str| net::glances_request(&set, p, &pass);
            glances::get_stats(&api, &set, !small, !small, None).ok()
        } else {
            None
        };
        // Glances answering means the server is up, so there's no need to wait for the other checks then
        let reach = s.is_some() || !r.join().unwrap_or_default().is_empty();
        (reach, s, g.join().ok().flatten())
    });
    out.reach = reach;
    out.stats = stats;
    if let Some((list, status, total)) = grid {
        out.total = total;
        out.up_count = status.values().filter(|v| **v).count();
        let cells = list
            .iter()
            .map(|c| {
                // stack dot: green = all up, orange = some down, red = all down
                let dot = if !set.widget_status_dots {
                    None
                } else {
                    Some(match c {
                        Item::Svc(s) => {
                            if status.get(&s.id).copied().unwrap_or(false) {
                                config::GREEN
                            } else {
                                config::RED
                            }
                        }
                        Item::Group(g) => {
                            let up = g.members.iter().filter(|m| status.get(&m.id).copied().unwrap_or(false)).count();
                            if up == g.members.len() {
                                config::GREEN
                            } else if up == 0 {
                                config::RED
                            } else {
                                config::ORANGE
                            }
                        }
                    })
                };
                Cell {
                    name: c.name().to_string(),
                    icon: c.icon().to_string(),
                    logo: if set.use_logos { logos.cached(c.logo()) } else { None },
                    dot,
                    click: match c {
                        Item::Svc(s) => Click::Url(s.url.clone()),
                        Item::Group(g) => Click::Stack(g.id.clone()),
                    },
                }
            })
            .collect();
        out.cells = Some(cells);
    }
    out
}

// ---------- drawing ----------

const ROUNDED: &str = "SF Pro Rounded, Nunito, Quicksand, Varela Round, Cantarell, sans-serif";
const MONO: &str = "SF Mono, JetBrains Mono, DejaVu Sans Mono, monospace";

fn layout(cr: &cairo::Context, text: &str, family: &str, px: f64, weight: gtk::pango::Weight) -> gtk::pango::Layout {
    let l = pangocairo::functions::create_layout(cr);
    let mut fd = gtk::pango::FontDescription::new();
    fd.set_family(family);
    fd.set_weight(weight);
    fd.set_absolute_size(px * gtk::pango::SCALE as f64);
    l.set_font_description(Some(&fd));
    l.set_text(text);
    l
}

fn text_size(l: &gtk::pango::Layout) -> (f64, f64) {
    let (_, r) = l.pixel_extents();
    (r.width() as f64, r.height() as f64)
}

fn show(cr: &cairo::Context, l: &gtk::pango::Layout, x: f64, y: f64, c: (f64, f64, f64), a: f64) {
    cr.set_source_rgba(c.0, c.1, c.2, a);
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, l);
    cr.new_path();
}

const WHITE: (f64, f64, f64) = (1.0, 1.0, 1.0);

/// Widget pill / button background. Classic: flat see-through fill. Glass: top-lit gradient + a thin rim.
#[allow(clippy::too_many_arguments)]
fn glass_box(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r_classic: f64, r_glass: f64, k: f64, flat: f64, glass: bool) {
    if !glass {
        super::draw::rounded_rect(cr, x, y, w, h, r_classic);
        cr.set_source_rgba(1.0, 1.0, 1.0, flat);
        let _ = cr.fill();
        return;
    }
    super::draw::rounded_rect(cr, x, y, w, h, r_glass);
    let g = cairo::LinearGradient::new(x, y, x + 0.4 * w, y + h);
    g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.2 * k);
    g.add_color_stop_rgba(0.55, 1.0, 1.0, 1.0, 0.06 * k);
    g.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.1 * k);
    let _ = cr.set_source(&g);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.22 * k);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

fn background(cr: &cairo::Context, w: f64, h: f64, d: &WidgetData) {
    cr.set_source_rgb(d.bg.0, d.bg.1, d.bg.2);
    let _ = cr.paint();
    if d.strength <= 0.0 {
        return;
    }
    // the 660 x 310 background picture of the iOS widget, filling the widget
    let s = (w / 660.0).max(h / 310.0);
    let (ox, oy) = ((w - 660.0 * s) / 2.0, (h - 310.0 * s) / 2.0);
    let glow = |x: f64, y: f64, r: f64, c: (f64, f64, f64)| {
        cr.new_path();
        for i in 0..28 {
            let rr = r * (1.0 - i as f64 / 28.0) * s;
            cr.set_source_rgba(c.0, c.1, c.2, 0.014 * d.strength);
            cr.arc(ox + x * s, oy + y * s, rr, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }
    };
    glow(90.0, 20.0, 280.0, d.g1);
    glow(620.0, 330.0, 320.0, d.g2);
}

fn ring(cr: &cairo::Context, x: f64, y: f64, size: f64, pct: f64, glass: bool) {
    cr.new_path();
    let c = hex_rgb(color_for(pct));
    let (cx, cy) = (x + size / 2.0, y + size / 2.0);
    let lw = size * 0.115;
    let r = size / 2.0 - lw * 1.1;
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.12);
    cr.set_line_width(lw);
    cr.arc(cx, cy, r, 0.0, 2.0 * PI);
    let _ = cr.stroke();
    let p = (pct.min(100.0) / 100.0).max(0.01);
    let a0 = -PI / 2.0;
    cr.set_line_cap(cairo::LineCap::Round);
    for (w, a) in [(lw * 1.9, 0.1), (lw * 1.45, 0.16), (lw, 1.0)] {
        cr.set_source_rgba(c.0, c.1, c.2, a);
        cr.set_line_width(w);
        cr.arc(cx, cy, r, a0, a0 + 2.0 * PI * p);
        let _ = cr.stroke();
    }
    if glass {
        let lr = r - lw * 0.95;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        cr.arc(cx, cy, lr, 0.0, 2.0 * PI);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.16);
        cr.set_line_width(1.0);
        let _ = cr.stroke();
    }
    let l = layout(cr, &format!("{}%", config::js_round(pct)), ROUNDED, size * 0.25, gtk::pango::Weight::Bold);
    let (tw, th) = text_size(&l);
    show(cr, &l, cx - tw / 2.0, cy - th / 2.0, WHITE, 1.0);
}

fn arrow(cr: &cairo::Context, x: f64, y: f64, s: f64, up: bool, diag: bool, c: (f64, f64, f64)) {
    cr.new_path();
    cr.set_source_rgb(c.0, c.1, c.2);
    cr.set_line_width(s * 0.16);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    if diag {
        cr.move_to(x + s * 0.15, y + s * 0.85);
        cr.line_to(x + s * 0.85, y + s * 0.15);
        cr.move_to(x + s * 0.35, y + s * 0.15);
        cr.line_to(x + s * 0.85, y + s * 0.15);
        cr.line_to(x + s * 0.85, y + s * 0.65);
    } else if up {
        cr.move_to(x + s / 2.0, y + s * 0.9);
        cr.line_to(x + s / 2.0, y + s * 0.1);
        cr.move_to(x + s * 0.15, y + s * 0.45);
        cr.line_to(x + s / 2.0, y + s * 0.1);
        cr.line_to(x + s * 0.85, y + s * 0.45);
    } else {
        cr.move_to(x + s / 2.0, y + s * 0.1);
        cr.line_to(x + s / 2.0, y + s * 0.9);
        cr.move_to(x + s * 0.15, y + s * 0.55);
        cr.line_to(x + s / 2.0, y + s * 0.9);
        cr.line_to(x + s * 0.85, y + s * 0.55);
    }
    let _ = cr.stroke();
}

fn logo_surface(png: &[u8]) -> Option<cairo::ImageSurface> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut surf = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).ok()?;
    let stride = surf.stride() as usize;
    {
        let mut data = surf.data().ok()?;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let p = img.get_pixel(x as u32, y as u32).0;
                let a = p[3] as u32;
                let pm = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                let o = y * stride + x * 4;
                // cairo ARGB32 is premultiplied, native endian (BGRA in memory on little endian)
                data[o] = pm(p[2]);
                data[o + 1] = pm(p[1]);
                data[o + 2] = pm(p[0]);
                data[o + 3] = p[3];
            }
        }
    }
    Some(surf)
}

/// A message that wraps.
fn message(cr: &cairo::Context, text: &str, x: f64, y: f64, w: f64, px: f64, a: f64) -> f64 {
    let l = layout(cr, text, config_font(), px, gtk::pango::Weight::Normal);
    l.set_width((w * gtk::pango::SCALE as f64) as i32);
    l.set_wrap(gtk::pango::WrapMode::WordChar);
    let (_, th) = text_size(&l);
    show(cr, &l, x, y, WHITE, a);
    th
}

fn config_font() -> &'static str {
    super::draw::FONT_FAMILY
}

/// The button grid of the large widget (and of medium widgets without Monitoring).
#[allow(clippy::too_many_arguments)]
fn grid(cr: &cairo::Context, x: f64, y: f64, inner: f64, cells: &[Cell], dots: bool, glass: bool, hits: &mut Vec<Hit>) -> f64 {
    let (cols, gap, ph) = (4usize, 5.0, 26.0);
    let pw = ((inner - gap * (cols as f64 - 1.0)) / cols as f64).floor();
    let space = if cols > 1 { (inner - pw * cols as f64) / (cols as f64 - 1.0) } else { 0.0 };
    let rows = cells.len().div_ceil(cols);
    for (i, c) in cells.iter().enumerate() {
        let (r, k) = (i / cols, i % cols);
        let px = x + k as f64 * (pw + space);
        let py = y + r as f64 * (ph + gap);
        glass_box(cr, px, py, pw, ph, 8.0, 9.0, 1.0, 0.1, glass);
        hits.push((px, py, pw, ph, c.click.clone()));
        // content, centred: dot, logo or emoji, name (shrinks to fit, then cut)
        let dot_w = if dots { 4.0 + 2.0 } else { 0.0 };
        let icon_w = 12.0 + 3.0;
        let avail = pw - 8.0 - dot_w - icon_w;
        let mut sz = 9.0;
        let mut l = layout(cr, &c.name, config_font(), sz, gtk::pango::Weight::Semibold);
        while text_size(&l).0 > avail && sz > 9.0 * 0.6 {
            sz -= 0.4;
            l = layout(cr, &c.name, config_font(), sz, gtk::pango::Weight::Semibold);
        }
        l.set_width((avail.max(1.0) * gtk::pango::SCALE as f64) as i32);
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let (tw, th) = text_size(&l);
        let total = dot_w + icon_w + tw.min(avail);
        let mut cx = px + (pw - total) / 2.0;
        let cy = py + ph / 2.0;
        if let (true, Some(col)) = (dots, c.dot) {
            cr.new_path();
            let cc = hex_rgb(col);
            cr.set_source_rgb(cc.0, cc.1, cc.2);
            cr.arc(cx + 2.0, cy, 2.0, 0.0, 2.0 * PI);
            let _ = cr.fill();
            cx += dot_w;
        }
        match c.logo.as_deref().and_then(logo_surface) {
            Some(s) => {
                cr.save().ok();
                let k = 12.0 / s.width().max(s.height()) as f64;
                cr.translate(cx, cy - 6.0);
                cr.scale(k, k);
                let _ = cr.set_source_surface(&s, 0.0, 0.0);
                cr.source().set_filter(cairo::Filter::Good);
                let _ = cr.paint();
                cr.restore().ok();
            }
            None => {
                let e = layout(cr, &c.icon, config_font(), 10.0, gtk::pango::Weight::Normal);
                let (ew, eh) = text_size(&e);
                show(cr, &e, cx + (12.0 - ew) / 2.0, cy - eh / 2.0, WHITE, 1.0);
            }
        }
        cx += icon_w;
        show(cr, &l, cx, cy - th / 2.0, WHITE, 1.0);
    }
    rows as f64 * ph + rows.saturating_sub(1) as f64 * gap
}

/// Draws a widget; returns the clickable places (x, y, w, h, what).
pub fn render(cr: &cairo::Context, fam: &str, d: &WidgetData) -> Vec<Hit> {
    let (w, h) = size_of(fam);
    let small = fam == "small";
    let large = fam == "large";
    let mut hits = vec![];
    background(cr, w, h, d);
    let (px, py) = (14.0, 12.0);
    let inner = w - 2.0 * px;
    let bottom = h - py;
    let title = layout(cr, &d.title, ROUNDED, 12.0, gtk::pango::Weight::Heavy);
    let (_, title_h) = text_size(&title);
    let header_h = title_h.max(19.0);

    // not set up yet: ask to open the app
    if !d.configured {
        show(cr, &title, px, py, WHITE, 1.0);
        let l = layout(cr, "Click to set up -board:\nenter your server's address in Settings.", config_font(), 12.0, gtk::pango::Weight::Normal);
        l.set_width((inner * gtk::pango::SCALE as f64) as i32);
        l.set_wrap(gtk::pango::WrapMode::WordChar);
        let (_, th) = text_size(&l);
        show(cr, &l, px, py + header_h + (bottom - py - header_h - th) / 2.0, WHITE, 0.75);
        return hits;
    }

    // header: name, status pill (time or OFFLINE), corner button
    let mut right = w - px;
    if !small && !d.link.is_empty() {
        let (bx, by) = (right - 24.0, py + (header_h - 24.0) / 2.0);
        glass_box(cr, bx, by, 24.0, 24.0, 12.0, 12.0, 1.3, 0.14, d.glass);
        arrow(cr, bx + 7.0, by + 7.0, 10.0, true, true, WHITE);
        hits.push((bx, by, 24.0, 24.0, Click::Url(d.link.clone())));
        right = bx - 6.0;
    }
    let pill_t = layout(cr, if d.reach { &d.time } else { "OFFLINE" }, MONO, 10.0, gtk::pango::Weight::Bold);
    let (ptw, pth) = text_size(&pill_t);
    let pw = 8.0 + 6.0 + 4.0 + ptw + 8.0;
    let ph = pth + 6.0;
    let (pxx, pyy) = (right - pw, py + (header_h - ph) / 2.0);
    glass_box(cr, pxx, pyy, pw, ph, 9.0, 10.0, 1.0, 0.1, d.glass);
    cr.new_path();
    let dc = hex_rgb(if d.reach { config::GREEN } else { config::RED });
    cr.set_source_rgb(dc.0, dc.1, dc.2);
    cr.arc(pxx + 8.0 + 3.0, pyy + ph / 2.0, 3.0, 0.0, 2.0 * PI);
    let _ = cr.fill();
    show(cr, &pill_t, pxx + 8.0 + 6.0 + 4.0, pyy + 3.0, WHITE, 1.0);
    let tw_max = (pxx - 6.0 - px).max(10.0);
    title.set_width((tw_max * gtk::pango::SCALE as f64) as i32);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    show(cr, &title, px, py + (header_h - title_h) / 2.0, WHITE, 1.0);
    let top = py + header_h;

    // Monitoring off: service buttons (medium / large) or a summary (small)
    if !d.mon {
        if small {
            let big = layout(cr, if d.reach { "ONLINE" } else { "OFFLINE" }, ROUNDED, 20.0, gtk::pango::Weight::Heavy);
            let (_, bh) = text_size(&big);
            let sub = if d.checked && d.total > 0 { Some(layout(cr, &format!("{} of {} services up", d.up_count, d.total), config_font(), 11.0, gtk::pango::Weight::Medium)) } else { None };
            let sh = sub.as_ref().map(|s| text_size(s).1 + 2.0).unwrap_or(0.0);
            let y = top + (bottom - top - bh - sh) / 2.0;
            show(cr, &big, px, y, hex_rgb(if d.reach { config::GREEN } else { config::RED }), 1.0);
            if let Some(s) = sub {
                show(cr, &s, px, y + bh + 2.0, WHITE, 0.75);
            }
        } else {
            let y = top + 10.0;
            match d.cells.as_ref().filter(|c| !c.is_empty()) {
                Some(cells) => {
                    grid(cr, px, y, inner, cells, d.dots, d.glass, &mut hits);
                }
                None => {
                    message(cr, if d.reach { "Add service buttons in the dashboard with +." } else { "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home." }, px, y, inner, 11.0, 0.7);
                }
            }
        }
        return hits;
    }

    let Some(stats) = d.stats.as_ref() else {
        let msg = if d.reach { "The server is up, but Glances isn't answering.\nCheck the Glances address in Settings > Monitoring." } else { "Can't reach the server.\nCheck the address in Settings, and your VPN if you're away from home." };
        if large {
            let cells = d.cells.clone().unwrap_or_default();
            let gh = if cells.is_empty() { 0.0 } else { (cells.len().div_ceil(4)) as f64 * 31.0 - 5.0 };
            let l = layout(cr, msg, config_font(), 11.0, gtk::pango::Weight::Normal);
            l.set_width((inner * gtk::pango::SCALE as f64) as i32);
            l.set_wrap(gtk::pango::WrapMode::WordChar);
            let (_, mh) = text_size(&l);
            let free = bottom - top - mh - gh;
            show(cr, &l, px, top + free / 3.0, WHITE, 0.7);
            if !cells.is_empty() {
                grid(cr, px, top + free / 3.0 * 2.0 + mh, inner, &cells, d.dots, d.glass, &mut hits);
            }
        } else {
            let l = layout(cr, msg, config_font(), 11.0, gtk::pango::Weight::Normal);
            l.set_width((inner * gtk::pango::SCALE as f64) as i32);
            l.set_wrap(gtk::pango::WrapMode::WordChar);
            let (_, mh) = text_size(&l);
            show(cr, &l, px, top + (bottom - top - mh) / 2.0, WHITE, 0.7);
        }
        return hits;
    };

    // ring gauges
    let mut items: Vec<(&str, f64)> = vec![("CPU", stats.cpu), ("RAM", stats.ram)];
    if let Some(s) = &stats.storage {
        items.push(("STORAGE", s.percent));
    }
    if small {
        items.truncate(2);
    }
    let rs = 54.0;
    let lab_h = 12.0;
    let rings_h = rs + 2.0 + lab_h;
    let rings_y = if small { top + 6.0 + (bottom - top - 6.0 - rings_h) / 2.0 } else { top + 6.0 };
    let n = items.len();
    for (i, (lab, pct)) in items.iter().enumerate() {
        let col_w = rs.max(40.0);
        let x = if n == 1 { px + (inner - col_w) / 2.0 } else { px + i as f64 * (inner - col_w) / (n as f64 - 1.0) };
        ring(cr, x + (col_w - rs) / 2.0, rings_y, rs, *pct, d.glass);
        let l = layout(cr, lab, ROUNDED, 9.0, gtk::pango::Weight::Bold);
        let (tw, _) = text_size(&l);
        show(cr, &l, x + (col_w - tw) / 2.0, rings_y + rs + 2.0, WHITE, 0.7);
    }
    if small {
        return hits;
    }
    // network at the bottom
    let t1 = layout(cr, &format!("{}/s", format_bytes(stats.net.down)), MONO, 11.0, gtk::pango::Weight::Medium);
    let t2 = layout(cr, &format!("{}/s", format_bytes(stats.net.up)), MONO, 11.0, gtk::pango::Weight::Medium);
    let (t1w, th) = text_size(&t1);
    let (t2w, _) = text_size(&t2);
    let nh = th + 8.0;
    let ny = bottom - nh;
    glass_box(cr, px, ny, inner, nh, 9.0, 11.0, 0.85, 0.08, d.glass);
    arrow(cr, px + 10.0, ny + (nh - 10.0) / 2.0, 10.0, false, false, hex_rgb(config::BLUE));
    show(cr, &t1, px + 10.0 + 14.0, ny + 4.0, WHITE, 1.0);
    let rx = px + inner - 10.0 - t2w;
    arrow(cr, rx - 14.0, ny + (nh - 10.0) / 2.0, 10.0, true, false, hex_rgb(config::PEACH));
    show(cr, &t2, rx, ny + 4.0, WHITE, 1.0);
    let _ = t1w;
    if large {
        if let Some(cells) = d.cells.as_ref().filter(|c| !c.is_empty()) {
            grid(cr, px, rings_y + rings_h + 10.0, inner, cells, d.dots, d.glass, &mut hits);
        }
    }
    hits
}

/// Draws a widget into a PNG file (scale 2 for sharp text).
pub fn render_png(d: &WidgetData, fam: &str, path: &Path) -> Result<(), String> {
    let (w, h) = size_of(fam);
    let s = 2.0;
    let surf = cairo::ImageSurface::create(cairo::Format::ARgb32, (w * s) as i32, (h * s) as i32).map_err(|e| e.to_string())?;
    {
        let cr = cairo::Context::new(&surf).map_err(|e| e.to_string())?;
        cr.scale(s, s);
        super::draw::rounded_rect(&cr, 0.0, 0.0, w, h, 22.0);
        cr.clip();
        render(&cr, fam, d);
    }
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    surf.write_to_png(&mut f).map_err(|e| e.to_string())
}

// ---------- the widget window ----------

struct Win {
    window: gtk::Window,
    area: gtk::DrawingArea,
    fam: RefCell<&'static str>,
    server: RefCell<Option<String>>,
    data: RefCell<Option<WidgetData>>,
    hits: RefCell<Vec<Hit>>,
    ctx: Rc<Ctx>,
    busy: std::cell::Cell<bool>,
    timer: RefCell<Option<glib::SourceId>>,
}

thread_local! {
    static WINDOWS: RefCell<Vec<Weak<Win>>> = const { RefCell::new(vec![]) };
}

/// Every open widget checks again (after Settings were saved).
pub fn refresh_all() {
    WINDOWS.with(|w| {
        for x in w.borrow().iter().filter_map(|x| x.upgrade()) {
            x.refresh();
        }
    });
}

impl Win {
    fn refresh(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        let path: PathBuf = self.ctx.data_path.clone();
        let (secrets, logos): (Arc<Secrets>, Arc<Logos>) = (self.ctx.secrets.clone(), self.ctx.logos.clone());
        let want = self.server.borrow().clone();
        let fam = *self.fam.borrow();
        let me = self.clone();
        bg(move || collect(&path, want.as_deref(), fam, &secrets, &logos), move |d| {
            me.busy.set(false);
            *me.data.borrow_mut() = Some(d);
            me.area.queue_draw();
            me.schedule();
        });
    }

    fn schedule(self: &Rc<Self>) {
        if let Some(t) = self.timer.borrow_mut().take() {
            t.remove();
        }
        let mins = Store::open(&self.ctx.data_path).settings().refresh_minutes.max(1) as u64;
        let me = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_secs(mins * 60), move || {
            if let Some(m) = me.upgrade() {
                m.timer.borrow_mut().take();
                m.refresh();
            }
        });
        *self.timer.borrow_mut() = Some(id);
    }

    fn set_family(self: &Rc<Self>, fam: &'static str) {
        *self.fam.borrow_mut() = fam;
        let (w, h) = size_of(fam);
        self.area.set_content_width(w as i32);
        self.area.set_content_height(h as i32);
        self.window.set_default_size(w as i32, h as i32);
        self.refresh();
    }

    fn click(self: &Rc<Self>, x: f64, y: f64) {
        let hit = self.hits.borrow().iter().find(|(hx, hy, hw, hh, _)| x >= *hx && x <= hx + hw && y >= *hy && y <= hy + hh).map(|h| h.4.clone());
        let server = self.data.borrow().as_ref().map(|d| d.server.clone());
        match hit.unwrap_or(Click::Dashboard) {
            Click::Url(u) => {
                gtk::UriLauncher::new(&u).launch(Some(&self.window), gtk::gio::Cancellable::NONE, |_| {});
            }
            Click::Stack(id) => super::open_dashboard(server, Some(id)),
            Click::Dashboard => super::open_dashboard(server, None),
        }
    }

    fn menu(self: &Rc<Self>, x: f64, y: f64) {
        let pop = gtk::Popover::new();
        pop.add_css_class("pmenu");
        pop.set_has_arrow(false);
        pop.set_parent(&self.area);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let item = |text: &str| {
            let btn = gtk::Button::with_label(text);
            if let Some(l) = btn.child().and_downcast::<gtk::Label>() {
                l.set_xalign(0.0);
            }
            b.append(&btn);
            btn
        };
        let me = self.clone();
        let p = pop.clone();
        item("Open dashboard").connect_clicked(move |_| {
            p.popdown();
            let s = me.data.borrow().as_ref().map(|d| d.server.clone());
            super::open_dashboard(s, None);
        });
        let me = self.clone();
        let p = pop.clone();
        item("Refresh now").connect_clicked(move |_| {
            p.popdown();
            me.refresh();
        });
        for (lab, fam) in [("Small", "small"), ("Medium", "medium"), ("Large", "large")] {
            let me = self.clone();
            let p = pop.clone();
            let cur = *self.fam.borrow() == fam;
            item(&format!("{}{lab}", if cur { "✓ " } else { "    " })).connect_clicked(move |_| {
                p.popdown();
                me.set_family(fam);
            });
        }
        let me = self.clone();
        let p = pop.clone();
        let close = item("Close widget");
        close.add_css_class("red");
        close.connect_clicked(move |_| {
            p.popdown();
            me.window.close();
        });
        pop.set_child(Some(&b));
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
    }
}

/// Opens a desktop widget window for a server (None = the first server).
pub fn open(ctx: &Rc<Ctx>, app: Option<&gtk::Application>, fam: &str, server: Option<String>) {
    let fam = family(fam);
    let window = gtk::Window::new();
    if let Some(a) = app {
        window.set_application(Some(a));
    }
    window.set_decorated(false);
    window.set_resizable(false);
    window.add_css_class("boardwidget");
    window.set_title(Some("-board widget"));
    window.set_icon_name(Some(super::APP_ID));
    let area = gtk::DrawingArea::new();
    let (w, h) = size_of(fam);
    area.set_content_width(w as i32);
    area.set_content_height(h as i32);
    window.set_child(Some(&area));
    let win = Rc::new(Win {
        window: window.clone(),
        area: area.clone(),
        fam: RefCell::new(fam),
        server: RefCell::new(server),
        data: RefCell::new(None),
        hits: RefCell::new(vec![]),
        ctx: ctx.clone(),
        busy: std::cell::Cell::new(false),
        timer: RefCell::new(None),
    });
    let w2 = Rc::downgrade(&win);
    area.set_draw_func(move |_, cr, _, _| {
        let Some(me) = w2.upgrade() else { return };
        let fam = *me.fam.borrow();
        let (w, h) = size_of(fam);
        super::draw::rounded_rect(cr, 0.0, 0.0, w, h, 22.0);
        cr.clip();
        let data = me.data.borrow().clone();
        match data.as_ref() {
            Some(d) => {
                *me.hits.borrow_mut() = render(cr, fam, d);
            }
            None => {
                // first paint while the data is on its way
                let st = Store::open(&me.ctx.data_path).settings();
                let t = config::theme(&st.theme);
                let c = hex_rgb(t.bg);
                cr.set_source_rgb(c.0, c.1, c.2);
                let _ = cr.paint();
                let l = layout(cr, "-board", ROUNDED, 15.0, gtk::pango::Weight::Heavy);
                show(cr, &l, 14.0, 12.0, WHITE, 0.8);
            }
        }
    });
    // click: open; drag: move the window; right-click: menu
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    let w2 = win.clone();
    let moved = Rc::new(std::cell::Cell::new(false));
    let m2 = moved.clone();
    click.connect_released(move |_, _, x, y| {
        if !m2.get() {
            w2.click(x, y);
        }
    });
    area.add_controller(click);
    let drag = gtk::GestureDrag::new();
    drag.set_button(gdk::BUTTON_PRIMARY);
    let m3 = moved.clone();
    drag.connect_drag_begin(move |_, _, _| m3.set(false));
    let w3 = win.clone();
    drag.connect_drag_update(move |g, dx, dy| {
        if moved.get() || dx.hypot(dy) < 6.0 {
            return;
        }
        moved.set(true);
        let Some((sx, sy)) = g.start_point() else { return };
        if let (Some(surf), Some(dev)) = (w3.window.surface(), g.device()) {
            if let Ok(top) = surf.downcast::<gdk::Toplevel>() {
                top.begin_move(&dev, gdk::BUTTON_PRIMARY as i32, sx, sy, g.current_event_time());
            }
        }
    });
    area.add_controller(drag);
    let rc = gtk::GestureClick::new();
    rc.set_button(gdk::BUTTON_SECONDARY);
    let w4 = win.clone();
    rc.connect_pressed(move |_, _, x, y| w4.menu(x, y));
    area.add_controller(rc);
    let w5 = win.clone();
    window.connect_close_request(move |_| {
        if let Some(t) = w5.timer.borrow_mut().take() {
            t.remove();
        }
        glib::Propagation::Proceed
    });
    WINDOWS.with(|w| {
        let mut v = w.borrow_mut();
        v.retain(|x| x.upgrade().is_some());
        v.push(Rc::downgrade(&win));
    });
    // keeps the widget alive while its window is open
    let keep = RefCell::new(Some(win.clone()));
    window.connect_destroy(move |_| {
        keep.borrow_mut().take();
    });
    window.present();
    win.refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_renders_every_size_and_state() {
        let base = WidgetData {
            server: "s1".into(),
            title: "MY SERVER".into(),
            configured: true,
            mon: true,
            reach: true,
            stats: Some(Stats { cpu: 12.0, ram: 64.0, disks: vec![], storage: Some(crate::glances::Storage { percent: 90.0, ..Default::default() }), net: crate::glances::Net { down: 2048.0, up: 10.0 } }),
            cells: Some(vec![Cell { name: "Jellyfin".into(), icon: "🎬".into(), logo: None, dot: Some(config::GREEN), click: Click::Url("http://x".into()) }; 6]),
            up_count: 6,
            total: 6,
            checked: true,
            link: "https://pve".into(),
            dots: true,
            time: "12:34".into(),
            bg: (0.0, 0.0, 0.1),
            g1: (0.1, 0.8, 0.6),
            g2: (0.4, 0.3, 1.0),
            strength: 1.0,
            glass: false,
        };
        let dir = tempfile::tempdir().unwrap();
        for fam in ["small", "medium", "large"] {
            for (mon, stats, configured, glass) in [(true, true, true, false), (true, true, true, true), (true, false, true, false), (false, false, true, false), (true, true, false, false)] {
                let mut d = base.clone();
                d.mon = mon;
                d.configured = configured;
                d.glass = glass;
                if !stats {
                    d.stats = None;
                }
                let p = dir.path().join(format!("{fam}.png"));
                render_png(&d, fam, &p).unwrap();
                assert!(std::fs::metadata(&p).unwrap().len() > 1000);
            }
        }
        // the large widget's buttons and the corner link are clickable
        let surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 364, 382).unwrap();
        let cr = cairo::Context::new(&surf).unwrap();
        let hits = render(&cr, "large", &base);
        assert_eq!(hits.iter().filter(|h| matches!(h.4, Click::Url(ref u) if u == "http://x")).count(), 6);
        assert!(hits.iter().any(|h| matches!(h.4, Click::Url(ref u) if u == "https://pve")));
        let hits = render(&cr, "small", &base);
        assert!(hits.is_empty(), "the small widget has no buttons or corner link");
    }
}
