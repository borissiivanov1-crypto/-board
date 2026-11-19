//! The GTK4 / libadwaita user interface.

pub mod app;
pub mod dialogs;
pub mod draw;
pub mod modal;
pub mod settings_ui;
pub mod splash;
pub mod style;
pub mod widget;

use crate::logos::Logos;
use crate::secrets::Secrets;
use crate::settings::Look;
use crate::store::Store;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub const APP_ID: &str = "io.github.board.Board";

/// What every window shares: the data file, saved logins, logos.
pub struct Ctx {
    pub store: RefCell<Store>,
    pub data_path: PathBuf,
    pub secrets: Arc<Secrets>,
    pub logos: Arc<Logos>,
    pub textures: RefCell<HashMap<String, gdk::Texture>>,
}

impl Ctx {
    pub fn new() -> Ctx {
        let (data, cache) = dirs();
        let data_path = data.join("board-data.json");
        Ctx {
            store: RefCell::new(Store::open(&data_path)),
            data_path,
            secrets: Arc::new(Secrets::open(data.join("secrets.json"))),
            logos: Arc::new(Logos::new(cache.join("logos"))),
            textures: RefCell::default(),
        }
    }

    /// A saved logo as a texture (kept in memory once read).
    pub fn texture(&self, url: &str) -> Option<gdk::Texture> {
        if url.is_empty() {
            return None;
        }
        if let Some(t) = self.textures.borrow().get(url) {
            return Some(t.clone());
        }
        let bytes = self.logos.cached(url)?;
        let t = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)).ok()?;
        self.textures.borrow_mut().insert(url.to_string(), t.clone());
        Some(t)
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the data and the cache live. BOARD_DATA_DIR puts both in one folder (for testing or a portable copy).
pub fn dirs() -> (PathBuf, PathBuf) {
    if let Ok(d) = std::env::var("BOARD_DATA_DIR") {
        let d = PathBuf::from(d);
        return (d.clone(), d.join("cache"));
    }
    (glib::user_data_dir().join("board"), glib::user_cache_dir().join("board"))
}

/// Runs blocking work on a worker thread and hands the result to the main thread.
pub fn bg<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    let h = gio::spawn_blocking(f);
    glib::spawn_future_local(async move {
        if let Ok(v) = h.await {
            done(v);
        }
    });
}

thread_local! {
    static CSS: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static CTX: RefCell<Option<Rc<Ctx>>> = const { RefCell::new(None) };
    static DASH: RefCell<Option<Rc<app::App>>> = const { RefCell::new(None) };
    static GTK_APP: RefCell<Option<adw::Application>> = const { RefCell::new(None) };
}

/// Applies a look's CSS to every window.
pub fn load_css(look: &Look) {
    let css = style::css(look);
    CSS.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_none() {
            let p = gtk::CssProvider::new();
            if let Some(d) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(&d, &p, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
            }
            *c = Some(p);
        }
        c.as_ref().unwrap().load_from_string(&css);
    });
}

fn ctx() -> Rc<Ctx> {
    CTX.with(|c| c.borrow_mut().get_or_insert_with(|| Rc::new(Ctx::new())).clone())
}

/// Shows the dashboard (on a server, with a stack open); opens it when it isn't open yet.
pub fn open_dashboard(server: Option<String>, stack: Option<String>) {
    let existing = DASH.with(|d| d.borrow().clone()).filter(|a| a.win.is_visible() || a.win.application().is_some());
    match existing {
        Some(a) => {
            if let Some(s) = server {
                let id = a.ctx.store.borrow_mut().initial_server(Some(&s), false);
                if id != a.st.borrow().cur {
                    a.go_server(&id, 0);
                }
            }
            a.win.present();
            if let Some(id) = stack {
                a.focus_stack(&id);
            }
        }
        None => {
            let Some(gapp) = GTK_APP.with(|g| g.borrow().clone()) else { return };
            let a = app::App::new(&gapp, ctx(), server);
            let a2 = a.clone();
            a.win.connect_close_request(move |_| {
                DASH.with(|d| {
                    let mut d = d.borrow_mut();
                    if d.as_ref().map(|x| Rc::ptr_eq(x, &a2)).unwrap_or(false) {
                        *d = None;
                    }
                });
                glib::Propagation::Proceed
            });
            DASH.with(|d| *d.borrow_mut() = Some(a.clone()));
            a.present(stack);
        }
    }
}

#[derive(Default, Debug)]
struct Args {
    server: Option<String>,
    stack: Option<String>,
    widget: Option<String>,
    screenshot: Option<String>,
    settings: bool,
}

fn parse(args: &[String]) -> Args {
    let mut a = Args::default();
    let mut i = 1;
    while i < args.len() {
        let next = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--server" | "-s" => {
                a.server = next;
                i += 1;
            }
            "--stack" => {
                a.stack = next;
                i += 1;
            }
            "--widget" | "-w" => {
                let size = next.filter(|n| ["small", "medium", "large"].contains(&n.as_str()));
                if size.is_some() {
                    i += 1;
                }
                a.widget = Some(size.unwrap_or_else(|| "medium".into()));
            }
            "--screenshot" => {
                a.screenshot = next;
                i += 1;
            }
            "--settings" => a.settings = true,
            _ => {}
        }
        i += 1;
    }
    a
}

const HELP: &str = "-board: a live home-server dashboard and desktop widget

Usage: board [OPTIONS]

  -s, --server NAME|NUMBER   show this server (its name, or 1, 2 ...)
      --stack ID             open with this stack unfolded
  -w, --widget [SIZE]        open a desktop widget: small, medium (default) or large
      --render-widget FILE   draw a widget to a PNG file and quit (with --size and --server)
      --size SIZE            widget size for --render-widget
      --settings             open Settings
  -h, --help                 this help
";

/// Saves a picture of a window (for checking the look without a screen).
fn screenshot(w: &impl IsA<gtk::Widget>, path: &str) -> Result<(), String> {
    let p = gtk::WidgetPaintable::new(Some(w));
    let (width, height) = (w.width() as f64, w.height() as f64);
    let snap = gtk::Snapshot::new();
    p.snapshot(&snap, width, height);
    let node = snap.to_node().ok_or("nothing drawn")?;
    let renderer = w.native().ok_or("no window")?.renderer().ok_or("no renderer")?;
    let tex = renderer.render_texture(&node, None);
    tex.save_to_png(path).map_err(|e| e.to_string())
}

pub fn run() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{HELP}");
        return glib::ExitCode::SUCCESS;
    }
    // draw a widget to a file, no window needed
    if let Some(i) = args.iter().position(|a| a == "--render-widget") {
        let Some(out) = args.get(i + 1) else {
            eprintln!("--render-widget needs a file name");
            return glib::ExitCode::FAILURE;
        };
        let get = |k: &str| args.iter().position(|a| a == k).and_then(|j| args.get(j + 1)).cloned();
        let fam = widget::family(&get("--size").unwrap_or_else(|| "medium".into()));
        let (data, cache) = dirs();
        let secrets = Secrets::open(data.join("secrets.json"));
        let logos = Logos::new(cache.join("logos"));
        let d = widget::collect(&data.join("board-data.json"), get("--server").as_deref(), fam, &secrets, &logos);
        return match widget::render_png(&d, fam, std::path::Path::new(out)) {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                glib::ExitCode::FAILURE
            }
        };
    }

    let gapp = adw::Application::builder().application_id(APP_ID).flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE).build();
    GTK_APP.with(|g| *g.borrow_mut() = Some(gapp.clone()));
    gapp.connect_startup(|_| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        if let Some(t) = gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d)) {
            // the app icon when running from the build folder
            let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/icons");
            if local.exists() {
                t.add_search_path(local);
            }
        }
        let look = ctx().store.borrow_mut().settings().look();
        load_css(&look);
    });
    gapp.connect_command_line(|g, cl| {
        let argv: Vec<String> = cl.arguments().iter().map(|a| a.to_string_lossy().to_string()).collect();
        let a = parse(&argv);
        if let Some(size) = &a.widget {
            let server = a.server.clone().or_else(|| DASH.with(|d| d.borrow().as_ref().map(|x| x.st.borrow().cur.clone())));
            widget::open(&ctx(), Some(g.upcast_ref()), size, server);
            return 0.into();
        }
        open_dashboard(a.server.clone(), a.stack.clone());
        if a.settings {
            if let Some(d) = DASH.with(|d| d.borrow().clone()) {
                settings_ui::open(&d);
            }
        }
        // for checking screens without a person: BOARD_DEBUG_DO=settings,expand:cpu,widget,group:<id>,add,power,addserver,menu
        if let Ok(acts) = std::env::var("BOARD_DEBUG_DO") {
            glib::timeout_add_local_once(std::time::Duration::from_millis(1800), move || {
                let Some(d) = DASH.with(|d| d.borrow().clone()) else { return };
                for (i, act) in acts.split(',').enumerate() {
                    let act = act.to_string();
                    let d = d.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(700 * i as u64), move || {
                    let act = act.as_str();
                    let (k, v) = act.split_once(':').unwrap_or((act, ""));
                    match k {
                        "settings" => settings_ui::open(&d),
                        "expand" => d.toggle_card(v),
                        "widget" => {
                            d.st.borrow_mut().widget_mode = true;
                            d.draw(true);
                        }
                        "group" => d.focus_stack(v),
                        "add" => dialogs::open_add(&d, None),
                        "updates" => dialogs::open_updates(&d),
                        "zpick" => dialogs::open_zima_pick(&d),
                        "logs" => {
                            let sv = d.st.borrow().layout.all.first().cloned();
                            if let Some(z) = sv.and_then(|sv| d.z_app_for(&sv)) {
                                dialogs::open_logs(&d, &z);
                            }
                        }
                        "power" => dialogs::open_power(&d, "restart", "", false),
                        "addserver" => dialogs::open_add_server(&d),
                        "pmenu" => d.w.pmenu.popup(),
                        "sscroll" => d.modals.scroll_top(v.parse().unwrap_or(0.0)),
                        "glass" => {
                            let mut l = d.st.borrow().look.clone();
                            l.style = "glass".into();
                            d.apply_look(&l);
                        }
                        "scroll" => {
                            let adj = d.w.scroller.vadjustment();
                            adj.set_value(v.parse().unwrap_or(0.0));
                        }
                        "menu" => {
                            let sv = d.st.borrow().layout.all.first().cloned();
                            if let Some(sv) = sv {
                                dialogs::open_menu(&d, &sv);
                            }
                        }
                        _ => {}
                    }
                    });
                }
            });
        }
        if let Some(path) = a.screenshot {
            let g2 = g.clone();
            let ms = std::env::var("BOARD_SCREENSHOT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(4500);
            glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
                if let Some(d) = DASH.with(|d| d.borrow().clone()) {
                    match screenshot(&d.w.root, &path) {
                        Ok(()) => println!("saved {path}"),
                        Err(e) => eprintln!("screenshot: {e}"),
                    }
                }
                g2.quit();
            });
        }
        0.into()
    });
    gapp.run_with_args(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line() {
        let s = |v: &[&str]| parse(&v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        let a = s(&["board", "--server", "2", "--widget", "large"]);
        assert_eq!(a.server.as_deref(), Some("2"));
        assert_eq!(a.widget.as_deref(), Some("large"));
        let a = s(&["board", "-w", "--server", "NAS"]);
        assert_eq!(a.widget.as_deref(), Some("medium"));
        assert_eq!(a.server.as_deref(), Some("NAS"));
        let a = s(&["board", "--stack", "g:1", "--settings"]);
        assert_eq!(a.stack.as_deref(), Some("g:1"));
        assert!(a.settings);
    }
}
