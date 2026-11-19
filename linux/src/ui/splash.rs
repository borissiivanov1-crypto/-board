//! The startup animation (Settings > Theme & look > Startup animation): a glass orb with the -b logo,
//! a ring drawing itself in the accent colour, and the name, then everything zooms away. Click to skip.

use super::app::App;
use super::draw::{draw_markup, ease_soft, ease_spring, rounded_rect, FONT_FAMILY};
use crate::config::hex_rgb;
use crate::settings::look_vars;
use gtk::cairo;
use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Instant;

const SHOW: f64 = 1.35;
const OUT: f64 = 0.9;

fn k(t: f64, delay: f64, dur: f64) -> f64 {
    ((t - delay) / dur).clamp(0.0, 1.0)
}

pub fn show(app: &Rc<App>) {
    let look = app.st.borrow().look.clone();
    let v = look_vars(&look);
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    let t0 = Instant::now();
    // when the zoom-out started (None = still showing)
    let out_at: Rc<Cell<Option<f64>>> = Rc::new(Cell::new(None));
    let bg = hex_rgb(&v.bg);
    let acc = hex_rgb(&v.acc);
    let rgb = |c: (u8, u8, u8)| (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0);
    let (g1, g2, glowop) = (rgb(v.g1), rgb(v.g2), v.glowop);
    let o2 = out_at.clone();
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        let t = t0.elapsed().as_secs_f64();
        let (alpha, zoom) = match o2.get() {
            Some(s) => {
                let p = ease_soft(k(t, s, OUT));
                (1.0 - ((t - s) / 0.7).clamp(0.0, 1.0), 1.0 + 0.25 * p)
            }
            None => (1.0, 1.0),
        };
        cr.push_group();
        cr.translate(w / 2.0, h / 2.0);
        cr.scale(zoom, zoom);
        cr.translate(-w / 2.0, -h / 2.0);
        cr.set_source_rgb(bg.0, bg.1, bg.2);
        let _ = cr.paint();
        // glows
        let vmax = w.max(h) / 100.0;
        let gp = ease_soft(k(t, 0.0, 2.4));
        let gs = 0.4 + 0.6 * gp;
        for (cx, cy, c) in [(-50.0 * vmax + 60.0 * vmax, -60.0 * vmax + 60.0 * vmax, g1), (w + 50.0 * vmax - 60.0 * vmax, h + 60.0 * vmax - 60.0 * vmax, g2)] {
            let r = 60.0 * vmax * gs * 1.414 * 0.6;
            let g = cairo::RadialGradient::new(cx, cy, 0.0, cx, cy, r);
            let a = (glowop * 1.3).min(1.0) * 0.7 * gp;
            g.add_color_stop_rgba(0.0, c.0, c.1, c.2, a);
            g.add_color_stop_rgba(1.0, c.0, c.1, c.2, 0.0);
            let _ = cr.set_source(&g);
            let _ = cr.paint();
        }
        // the orb
        let (cx, cy) = (w / 2.0, h / 2.0 - 22.0);
        let op = k(t, 0.0, 1.0);
        let os = ease_spring(op);
        let scale = 0.2 + 0.8 * os;
        let rot = -25.0 * (1.0 - os) * PI / 180.0;
        let radius = 69.0 + (42.0 - 69.0) * os.min(1.0);
        cr.save().ok();
        cr.translate(cx, cy);
        cr.rotate(rot);
        cr.scale(scale, scale);
        cr.push_group();
        rounded_rect(cr, -69.0, -69.0, 138.0, 138.0, radius);
        let fill = cairo::LinearGradient::new(-69.0, -69.0, 40.0, 80.0);
        fill.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.22);
        fill.add_color_stop_rgba(0.55, 1.0, 1.0, 1.0, 0.04);
        fill.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.10);
        let _ = cr.set_source(&fill);
        let _ = cr.fill_preserve();
        let rim = cairo::LinearGradient::new(-69.0, -69.0, 69.0, 69.0);
        rim.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.8);
        rim.add_color_stop_rgba(0.35, 1.0, 1.0, 1.0, 0.1);
        rim.add_color_stop_rgba(0.55, 1.0, 1.0, 1.0, 0.0);
        rim.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.45);
        let _ = cr.set_source(&rim);
        cr.set_line_width(1.2);
        let _ = cr.stroke();
        // the light sweeping across
        let sp = ease_soft(k(t, 0.55, 1.1));
        if sp > 0.0 && sp < 1.0 {
            rounded_rect(cr, -69.0, -69.0, 138.0, 138.0, radius);
            cr.clip();
            let x = -69.0 - 110.0 + 138.0 * 2.3 * sp;
            let sw = cairo::LinearGradient::new(x, 0.0, x + 83.0, 0.0);
            sw.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
            sw.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.35);
            sw.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
            let _ = cr.set_source(&sw);
            let _ = cr.paint();
            cr.reset_clip();
        }
        // the ring draws itself
        let rr = 48.0 * 112.0 / 120.0;
        cr.set_line_width(8.0 * 112.0 / 120.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.12);
        cr.arc(0.0, 0.0, rr, 0.0, 2.0 * PI);
        let _ = cr.stroke();
        let ap = ease_soft(k(t, 0.2, 1.0)) * (301.6 - 52.0) / 301.6;
        if ap > 0.001 {
            cr.set_line_cap(cairo::LineCap::Round);
            for (lw, a) in [(18.0, 0.12), (12.0, 0.25)] {
                cr.set_line_width(lw);
                cr.set_source_rgba(acc.0, acc.1, acc.2, a);
                cr.arc(0.0, 0.0, rr, -PI / 2.0, -PI / 2.0 + 2.0 * PI * ap);
                let _ = cr.stroke();
            }
            cr.set_line_width(8.0 * 112.0 / 120.0);
            cr.set_source_rgb(acc.0, acc.1, acc.2);
            cr.arc(0.0, 0.0, rr, -PI / 2.0, -PI / 2.0 + 2.0 * PI * ap);
            let _ = cr.stroke();
        }
        // -b
        let bp = k(t, 0.4, 0.7);
        if bp > 0.0 {
            let bs = 0.4 + 0.6 * ease_spring(bp);
            cr.save().ok();
            cr.scale(bs, bs);
            cr.push_group();
            draw_markup(cr, "<span font_weight=\"900\" font_size=\"30pt\" letter_spacing=\"-2048\" foreground=\"#ffffff\">-b</span>", 0.0, 0.0, FONT_FAMILY);
            let _ = cr.pop_group_to_source();
            let _ = cr.paint_with_alpha(ease_soft(bp));
            cr.restore().ok();
        }
        let _ = cr.pop_group_to_source();
        let _ = cr.paint_with_alpha(op.min(1.0) * 1.0f64.min(op * 3.0));
        cr.restore().ok();
        // the name
        let np = ease_soft(k(t, 0.45, 1.0));
        if np > 0.0 {
            let ls = (0.34 + (1.1 - 0.34) * (1.0 - np)) * 14.0 * 1024.0;
            let m = format!("<span font_weight=\"800\" font_size=\"10.5pt\" letter_spacing=\"{}\" foreground=\"#ffffff\">-BOARD</span>", ls.round() as i64);
            cr.push_group();
            draw_markup(cr, &m, w / 2.0, cy + 69.0 + 26.0 + 9.0, FONT_FAMILY);
            let _ = cr.pop_group_to_source();
            let _ = cr.paint_with_alpha(np * 0.85);
        }
        let _ = cr.pop_group_to_source();
        let _ = cr.paint_with_alpha(alpha.max(0.0));
    });
    app.w.root.add_overlay(&area);
    let end = {
        let (o, root, a) = (out_at.clone(), app.w.root.clone(), area.clone());
        move |now: f64| {
            if o.get().is_none() {
                o.set(Some(now));
            }
            let _ = (&root, &a);
        }
    };
    let click = gtk::GestureClick::new();
    let e2 = end.clone();
    click.connect_pressed(move |_, _, _, _| e2(t0.elapsed().as_secs_f64()));
    area.add_controller(click);
    let root = app.w.root.clone();
    area.add_tick_callback(move |a, _| {
        let t = t0.elapsed().as_secs_f64();
        if out_at.get().is_none() && t >= SHOW {
            out_at.set(Some(t));
        }
        a.queue_draw();
        if let Some(s) = out_at.get() {
            if t - s >= OUT {
                root.remove_overlay(a);
                return glib::ControlFlow::Break;
            }
        }
        glib::ControlFlow::Continue
    });
}
