//! The look: GTK CSS made from the theme, accent, glow, card style, corners, font and size settings.
//! Classic and Liquid Glass follow the original styles (GTK has no backdrop blur, so the glass panels
//! are a little more opaque).

use crate::settings::{look_vars, Look};

const BASE: &str = r#"
window.board, window.board.background { background-color: @bg@; color: #ffffff; }
.board { font-family: @font@; }
.board * { -gtk-secondary-caret-color: @acc@; }
.board headerbar { background: none; box-shadow: none; border: none; min-height: 36px; }
.board headerbar windowcontrols button { background-color: rgba(255,255,255,.08); color: rgba(255,255,255,.8); }
.board scrollbar { background: none; border: none; }
.board scrollbar slider { background-color: rgba(255,255,255,.25); min-width: 4px; }
.board label { color: #ffffff; }
.board button { color: #ffffff; text-shadow: none; -gtk-icon-shadow: none; }
.tnum { font-feature-settings: "tnum"; }

/* header */
.h1 { font-size: 22px; font-weight: 800; letter-spacing: 2.6px; }
.tw { border-radius: 12px; padding: 2px 4px; }
.tw:hover { background-color: rgba(255,255,255,.04); }
.tchev { opacity: .6; }
.live { font-size: 12px; font-weight: 700; letter-spacing: 1.2px; padding: 6px 12px; border-radius: 99px; background-color: rgba(255,255,255,.1); }
.dot { min-width: 8px; min-height: 8px; border-radius: 99px; background-color: #34e0a1; box-shadow: 0 0 6px rgba(52,224,161,.7); }
.dot.off { background-color: #ff5d73; box-shadow: 0 0 6px rgba(255,93,115,.7); }
.dot.wait { background-color: rgba(255,255,255,.35); box-shadow: none; }
button.gear { min-width: 32px; min-height: 32px; border-radius: 99px; padding: 0; background: rgba(255,255,255,.1); border: none; box-shadow: none; }
button.gear:hover { background: rgba(255,255,255,.16); }
button.gear:active { background: rgba(255,255,255,.2); }
.sdot { min-width: 7px; min-height: 7px; border-radius: 4px; background-color: rgba(255,255,255,.28); transition: min-width 300ms, background-color 300ms; }
.sdot.on { min-width: 20px; background-color: @acc@; }
button.sdotbtn { padding: 4px 0; min-height: 0; min-width: 0; background: none; border: none; box-shadow: none; }
button.splus { font-size: 15px; font-weight: 600; opacity: .45; padding: 0 4px; min-height: 0; min-width: 0; background: none; border: none; box-shadow: none; }
button.splus:hover { opacity: .8; }

/* monitoring */
button.monbtn, .monbtn { padding: 14px 18px; border-radius: 20px; border: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,@glass@); color: #fff; font-size: 13px; font-weight: 800; letter-spacing: 1.8px; box-shadow: none; min-height: 0; }
button.monbtn:hover { background: rgba(255,255,255,@glassh@); }
button.monbtn:active { background: rgba(255,255,255,.14); }
.monst { font-size: 10px; letter-spacing: 1px; color: #ffb454; opacity: .9; font-weight: 800; }
.chev { opacity: .8; transition: -gtk-icon-transform 350ms; }
.tchev { transition: -gtk-icon-transform 300ms; }
.monbtn.open .chev, .tw.open .tchev { -gtk-icon-transform: rotate(180deg); }
.card { background-color: rgba(255,255,255,@glass@); border: 1px solid rgba(255,255,255,.1); border-radius: 24px; padding: 18px 12px 14px; }
.card:hover { background-color: rgba(255,255,255,@glassh@); }
.lbl { font-size: 12px; font-weight: 700; letter-spacing: 1.7px; opacity: .7; }
.netcard { background-color: rgba(255,255,255,@glass@); border: 1px solid rgba(255,255,255,.1); border-radius: 24px; padding: 18px 10px; }
.netv { font-size: 20px; font-weight: 700; font-feature-settings: "tnum"; }
.netc { font-size: 11px; letter-spacing: 1.5px; opacity: .65; font-weight: 700; }
.arrow { font-size: 20px; font-weight: 700; }
.arrow.dn { color: #5ac8fa; }
.arrow.up { color: #ff9f6b; }

/* enlarged card details */
.bigval { font-size: 24px; font-weight: 800; font-feature-settings: "tnum"; }
.ds { background-color: rgba(255,255,255,.06); border-radius: 14px; padding: 10px 6px; }
.dsv { font-size: 14px; font-weight: 700; font-feature-settings: "tnum"; }
.dsl { font-size: 10px; letter-spacing: 1.2px; opacity: .55; font-weight: 700; }
.dh { font-size: 11px; font-weight: 700; letter-spacing: 1.5px; opacity: .6; }
.gbox { background-color: rgba(255,255,255,.05); border-radius: 16px; padding: 10px; }
.gcap { font-size: 10px; opacity: .5; letter-spacing: .6px; }
.gwait { font-size: 12px; opacity: .5; }
.pn { font-size: 13px; font-weight: 600; }
.pv { font-size: 12px; opacity: .85; font-feature-settings: "tnum"; }
.drv { background-color: rgba(255,255,255,.05); border-radius: 16px; padding: 12px; }
.drvn { font-size: 15px; font-weight: 700; }
.drvs { font-size: 12px; opacity: .7; font-feature-settings: "tnum"; }
.hint { font-size: 10px; letter-spacing: 1.4px; opacity: .35; font-weight: 700; }

/* services */
.sec { font-size: 12px; font-weight: 700; letter-spacing: 1.7px; opacity: .6; }
button.lay { font-size: 12px; font-weight: 700; color: #fff; background: rgba(255,255,255,.12); border: 1px solid rgba(255,255,255,.14); border-radius: 99px; padding: 6px 12px; min-height: 0; box-shadow: none; }
button.lay.on { background: @acc@; color: @ink@; border-color: transparent; }
button.lay.on label { color: @ink@; }
button.svc { min-width: 0; min-height: 0; padding: 14px; border-radius: 20px; color: #fff; background: rgba(255,255,255,@glass@); border: 1px solid rgba(255,255,255,.1); box-shadow: none; }
button.svc:hover { background: rgba(255,255,255,@glassh@); }
button.svc:active { background: rgba(255,255,255,.18); }
.svc .ic { font-size: 22px; }
.nm { font-size: 15px; font-weight: 700; }
.pt { font-size: 11px; opacity: .55; font-feature-settings: "tnum"; }
.sd { min-width: 8px; min-height: 8px; border-radius: 99px; background-color: rgba(255,255,255,.25); }
.sd.up { background-color: #34e0a1; box-shadow: 0 0 6px rgba(52,224,161,.8); }
.sd.dn { background-color: #ff5d73; box-shadow: 0 0 6px rgba(255,93,115,.8); }
.sd.part { background-color: #ffb454; box-shadow: 0 0 6px rgba(255,180,84,.8); }
.lifted { opacity: .25; }
.dropon { box-shadow: inset 0 0 0 2px @acc@; }
.svc.over { opacity: .4; border-style: dashed; }
.grph .gc { font-size: 11px; opacity: .5; font-weight: 700; letter-spacing: 0; }
.grph .gname { font-size: 13px; font-weight: 800; letter-spacing: 0; }
.gbody-empty, .hid-empty, .svc-empty { font-size: 12px; opacity: .5; }
.hidbox { min-height: 46px; padding: 6px; border: 1px dashed rgba(255,255,255,.16); border-radius: 14px; }
.emptybox { min-height: 90px; padding: 16px; border: 1px dashed rgba(255,255,255,.16); border-radius: 20px; }
.emptybox label { font-size: 13px; opacity: .6; }
.wide button.svc { padding: 0 6px; min-height: 34px; border-radius: 10px; }
.wide .svc .ic { font-size: 13px; }
.wide .nm { font-size: 10px; }
.wide .sd { min-width: 6px; min-height: 6px; box-shadow: none; }
.hidbox button.svc { opacity: .7; }
.wnote { font-size: 12px; opacity: .55; }
button.addbtn { min-width: 52px; min-height: 52px; border-radius: 16px; border: 1px solid rgba(255,255,255,.14); background: rgba(255,255,255,.1); color: #fff; font-size: 28px; font-weight: 300; padding: 0; box-shadow: none; }
button.addbtn:hover { background: rgba(255,255,255,.16); }
button.addbtn:active { background: rgba(255,255,255,.22); }

/* Reboot / Shut down menu */
popover.pmenu > contents { background-color: @panel2@; border: 1px solid rgba(255,255,255,.14); border-radius: 18px; padding: 6px; box-shadow: 0 20px 60px rgba(0,0,0,.5); }
popover.pmenu > arrow { background: none; border: none; }
popover.pmenu button { min-height: 42px; padding: 0 12px; border-radius: 12px; background: none; border: none; box-shadow: none; font-size: 15px; font-weight: 700; min-width: 170px; }
popover.pmenu button:hover { background: rgba(255,255,255,.08); }
popover.pmenu button:active { background: rgba(255,255,255,.12); }
popover.pmenu button.red, popover.pmenu button.red label, popover.pmenu button.red image { color: #ff5d73; }
popover.wmenu > contents { background-color: @panel2@; border: 1px solid rgba(255,255,255,.14); border-radius: 14px; }

/* toast */
.toast { padding: 10px 16px; border-radius: 18px; background-color: rgba(30,38,64,.96); border: 1px solid rgba(255,255,255,.14); box-shadow: 0 10px 30px rgba(0,0,0,.4); font-size: 13px; font-weight: 600; }

/* popups */
.scrim { background-color: rgba(5,10,20,.62); }
.modal { padding: 22px 18px 18px; border-radius: 30px; background-color: @panel@; border: 1px solid rgba(255,255,255,.14); box-shadow: 0 20px 60px rgba(0,0,0,.5); }
.h2 { font-size: 17px; font-weight: 800; letter-spacing: 1px; }
.h2sub { font-size: 10px; font-weight: 700; letter-spacing: 1.4px; opacity: .55; }
.flabel { font-size: 11px; font-weight: 700; letter-spacing: 1.3px; opacity: .6; margin-left: 4px; }
.in { background-color: rgba(255,255,255,.09); border: 1px solid rgba(255,255,255,.12); border-radius: 14px; padding: 0 12px; min-height: 44px; }
.in entry, .in text, .in password, entry.bare { background: none; border: none; box-shadow: none; outline: none; color: #fff; font-size: 16px; min-height: 42px; padding: 0; margin: 0; }
.in entry > text > placeholder, .in placeholder { color: rgba(255,255,255,.3); }
.in dropdown > button, .in dropdown button { background: none; border: none; box-shadow: none; padding: 0 4px 0 0; min-height: 40px; font-size: 16px; color: rgba(255,255,255,.85); }
.in image.lp { margin-left: 8px; }
.inpre { opacity: .55; font-size: 16px; }
popover.menu > contents, popover > contents { background-color: @panel2@; color: #fff; }
popover modelbutton:hover, popover row:hover, popover row:selected { background-color: rgba(255,255,255,.1); }
.tg label.flabel { margin-left: 0; }
.board switch { background-color: rgba(255,255,255,.18); border: none; border-radius: 14px; min-width: 46px; min-height: 28px; padding: 0; box-shadow: none; transition: background-color 200ms; }
.board switch:checked { background-color: @acc@; }
.board switch > slider { background-color: #fff; min-width: 22px; min-height: 22px; border-radius: 99px; margin: 3px; box-shadow: 0 1px 4px rgba(0,0,0,.35); border: none; }
.board switch image { color: transparent; -gtk-icon-size: 0; }
.err { font-size: 12px; color: #ff5d73; }
.cnote { font-size: 13px; opacity: .65; }
.fh { font-size: 11px; opacity: .45; margin: 0 4px; }
.btns button, button.bbtn { min-height: 44px; border: none; border-radius: 14px; font-size: 15px; font-weight: 700; color: #fff; background: rgba(255,255,255,.12); box-shadow: none; }
.btns button:hover { background: rgba(255,255,255,.16); }
.btns button.ok { background: @acc@; color: @ink@; }
.btns button.ok label { color: @ink@; }
.btns button.del { background: #ff5d73; color: #fff; }
.btns button.gtxt label { color: #34e0a1; }
.btns button.rtxt label { color: #ff5d73; }
button.chip { font-size: 12px; font-weight: 700; color: #fff; background: rgba(255,255,255,.14); border: none; border-radius: 99px; padding: 5px 10px; min-height: 0; min-width: 0; box-shadow: none; }
button.chip:hover { background: rgba(255,255,255,.2); }
button.chip:active { background: rgba(255,255,255,.26); }

/* Settings */
.shd button { min-height: 36px; padding: 0 14px; border: none; border-radius: 12px; font-size: 15px; font-weight: 700; color: #fff; background: rgba(255,255,255,.12); box-shadow: none; }
.shd button.ok { background: @acc@; }
.shd button.ok label { color: @ink@; }
.sdiv { font-size: 11px; font-weight: 800; letter-spacing: 1.8px; opacity: .5; }
.scard { padding: 14px; border-radius: 18px; background-color: rgba(255,255,255,.05); border: 1px solid rgba(255,255,255,.08); }
.shint { font-size: 11px; opacity: .45; margin: 0 4px; }
.srow-t { font-size: 14px; font-weight: 700; }
.srow-s { font-size: 11px; opacity: .5; }
.trow-h { font-size: 10px; font-weight: 700; letter-spacing: 1.2px; opacity: .5; }
.trow-n { font-size: 13px; opacity: .8; margin-left: 4px; font-weight: 700; }
.sres { font-size: 12px; }
.sres .rdot { min-width: 8px; min-height: 8px; border-radius: 99px; background-color: #34e0a1; margin-top: 4px; }
.sres .bad .rdot { background-color: #ff5d73; }
.sbtns button { min-height: 44px; border: none; border-radius: 14px; font-size: 15px; font-weight: 700; color: #fff; background: rgba(255,255,255,.12); box-shadow: none; }
.sbtns button:hover { background: rgba(255,255,255,.16); }
.sbtns button:active { background: rgba(255,255,255,.2); }
.sbtns button.rtxt label { color: #ff5d73; }
.sbtns button.del { background: rgba(255,93,115,.16); }
.sbtns button.del label { color: #ff5d73; }
.sbtns button:disabled { opacity: .4; }
.swait { font-size: 13px; opacity: .6; }
.sbusy { opacity: .5; }
.seg { padding: 3px; background-color: rgba(255,255,255,.09); border: 1px solid rgba(255,255,255,.12); border-radius: 14px; }
.seg button { min-height: 38px; border: none; border-radius: 11px; background: none; color: #fff; font-size: 14px; font-weight: 700; opacity: .75; box-shadow: none; padding: 0 6px; }
.seg button.on { background: @acc@; opacity: 1; }
.seg button.on label { color: @ink@; }
.seg .rd { min-width: 10px; min-height: 10px; border-radius: 99px; border: 2px solid rgba(255,255,255,.5); }
.seg button.on .rd { border: 4px solid @ink@; background-color: @acc@; min-width: 6px; min-height: 6px; }
button.swt { min-width: 44px; min-height: 44px; border-radius: 99px; border: 2px solid rgba(255,255,255,.15); padding: 0; background: none; box-shadow: none; }
button.swt.dashed { border-style: dashed; }
button.swt.on { border-color: #fff; box-shadow: 0 0 0 3px rgba(255,255,255,.18); }
.swn { font-size: 12px; opacity: .6; margin: 2px 4px 0; }
.slogin { padding: 12px; border-radius: 14px; background-color: rgba(255,255,255,.04); border: 1px solid rgba(255,255,255,.1); }

/* desktop widget */
window.boardwidget, window.boardwidget.background { background: none; box-shadow: none; }
.wframe { border-radius: 22px; }

/* ---- Sharper corners ---- */
.board.sharp .card, .board.sharp .netcard { border-radius: 12px; }
.board.sharp button.svc, .board.sharp button.monbtn, .board.sharp .scard { border-radius: 10px; }
.board.sharp.wide button.svc, .board.sharp .wide button.svc { border-radius: 6px; }
.board.sharp .modal { border-radius: 16px; }
.board.sharp .in, .board.sharp .btns button, .board.sharp .sbtns button, .board.sharp button.addbtn { border-radius: 8px; }

/* ---- 3 columns and compact buttons ---- */
.board.c3 .dash button.svc { padding: 12px; }
.board.c3 .dash .nm { font-size: 13px; }
.board.compact .dash button.svc { padding: 10px 12px; }
.board.compact .dash .svc .ic { font-size: 18px; }
.board.compact .dash .nm { font-size: 13px; }
"#;

const GLASS: &str = r#"
/* ===================== LIQUID GLASS ===================== */
.board.glass .h1 { font-size: 24px; letter-spacing: 2.4px; }
.board.glass .card, .board.glass .netcard, .board.glass button.svc, .board.glass button.monbtn, .board.glass .live, .board.glass button.gear,
.board.glass button.addbtn, .board.glass button.lay, .board.glass button.chip, .board.glass .btns button, .board.glass .sbtns button,
.board.glass .shd button, .board.glass .scard, .board.glass .seg {
  background-color: transparent;
  background-image: linear-gradient(165deg, rgba(255,255,255,@g23@) 0%, rgba(255,255,255,@g06@) 52%, rgba(255,255,255,@g13@) 100%);
  border: none;
  box-shadow: inset 0 1px 0 rgba(255,255,255,.30), inset 0 0 0 1px rgba(255,255,255,.10), inset 0 -1px 0 rgba(255,255,255,.05), 0 12px 30px -10px rgba(0,0,0,.5);
  transition: background-color 300ms, box-shadow 300ms, opacity 350ms;
}
.board.glass .card:hover, .board.glass button.svc:hover, .board.glass button.monbtn:hover, .board.glass button.addbtn:hover, .board.glass button.gear:hover {
  background-image: linear-gradient(165deg, rgba(255,255,255,@g28@) 0%, rgba(255,255,255,@g08@) 52%, rgba(255,255,255,@g16@) 100%);
}
.board.glass button.svc:active, .board.glass button.monbtn:active, .board.glass button.addbtn:active, .board.glass button.gear:active, .board.glass button.chip:active, .board.glass .sbtns button:active {
  background-image: radial-gradient(circle, rgba(255,255,255,.34), rgba(255,255,255,.08) 35%, rgba(255,255,255,0) 65%), linear-gradient(165deg, rgba(255,255,255,@g23@) 0%, rgba(255,255,255,@g06@) 52%, rgba(255,255,255,@g13@) 100%);
}
.board.glass .card, .board.glass .netcard { border-radius: 30px; }
.board.glass button.svc, .board.glass button.monbtn { border-radius: 24px; }
.board.glass .modal { border-radius: 36px; }
.board.glass .live { padding: 7px 13px; }
.board.glass button.gear { min-width: 36px; min-height: 36px; }
.board.glass button.addbtn { min-width: 58px; min-height: 58px; border-radius: 99px; font-size: 30px; }
.board.glass button.lay { padding: 7px 14px; }
.board.glass .seg { border-radius: 16px; }
.board.glass .seg button { border-radius: 13px; }
.board.glass button.lay.on, .board.glass .btns button.ok, .board.glass .shd button.ok, .board.glass .seg button.on {
  background-color: @acc@;
  background-image: linear-gradient(180deg, rgba(255,255,255,.42), rgba(255,255,255,0) 58%);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.5), 0 8px 22px -8px @acc@;
}
.board.glass .btns button.del {
  background-color: #ff5d73;
  background-image: linear-gradient(180deg, rgba(255,255,255,.38), rgba(255,255,255,0) 58%);
  box-shadow: inset 0 1px 0 rgba(255,255,255,.45), 0 8px 22px -8px #ff5d73;
}
.board.glass .btns button, .board.glass .sbtns button { min-height: 48px; border-radius: 16px; }
.board.glass .nm { font-weight: 600; }
.board.glass .wide button.svc, .board.glass.wide button.svc { border-radius: 12px; }
.board.glass .svc.over { opacity: .32; }
.board.glass .lifted { opacity: .15; }
.board.glass .modal, .board.glass popover.pmenu > contents, .board.glass .toast {
  background-color: @glasspanel@;
  background-image: linear-gradient(165deg, rgba(255,255,255,.17), rgba(255,255,255,.04) 55%, rgba(255,255,255,.08));
  border: none;
  box-shadow: inset 0 1px 0 rgba(255,255,255,.3), inset 0 0 0 1px rgba(255,255,255,.12), 0 30px 70px -10px rgba(0,0,0,.6);
}
.board.glass popover.pmenu > contents { border-radius: 22px; }
.board.glass popover.pmenu button { border-radius: 14px; }
.board.glass .toast { border-radius: 22px; padding: 12px 18px; }
.board.glass .scrim { background-color: rgba(0,0,0,.42); }
.board.glass .ds, .board.glass .gbox, .board.glass .drv { background-color: rgba(255,255,255,.06); box-shadow: inset 0 1px 0 rgba(255,255,255,.12), inset 0 0 0 1px rgba(255,255,255,.06); }
.board.glass .ds { border-radius: 16px; }
.board.glass .sd { transition: background-color 500ms, box-shadow 500ms; }
.board.glass .sd.up { box-shadow: 0 0 0 3px rgba(52,224,161,.16), 0 0 10px rgba(52,224,161,.9); }
.board.glass .sd.dn { box-shadow: 0 0 0 3px rgba(255,93,115,.16), 0 0 10px rgba(255,93,115,.9); }
.board.glass .sd.part { box-shadow: 0 0 0 3px rgba(255,180,84,.16), 0 0 10px rgba(255,180,84,.9); }
.board.glass .in { background-color: rgba(255,255,255,.06); border: none; border-radius: 16px; box-shadow: inset 0 1px 3px rgba(0,0,0,.3), inset 0 0 0 1px rgba(255,255,255,.14); transition: box-shadow 350ms, background-color 350ms; }
.board.glass .in:focus-within { background-color: rgba(255,255,255,.11); box-shadow: inset 0 1px 3px rgba(0,0,0,.25), 0 0 0 2px @acc@, 0 0 22px -4px @acc@; }
.board.glass switch { background-color: rgba(120,120,135,.32); box-shadow: inset 0 1px 3px rgba(0,0,0,.3), inset 0 0 0 1px rgba(255,255,255,.12); transition: background-color 400ms; }
.board.glass switch:checked { background-color: @acc@; }
.board.glass switch:active > slider { min-width: 30px; background-color: rgba(255,255,255,.75); }
.board.glass button.swt.on { box-shadow: 0 0 0 3px rgba(255,255,255,.18), 0 0 16px rgba(255,255,255,.2); }
"#;

/// The CSS for a look.
pub fn css(look: &Look) -> String {
    let v = look_vars(look);
    let g = v.glass;
    let font = match look.font.as_str() {
        "rounded" => "\"SF Pro Rounded\", \"Nunito\", \"Quicksand\", \"Varela Round\", \"M PLUS Rounded 1c\", \"Comfortaa\", \"Cantarell\", sans-serif",
        "serif" => "\"New York\", \"Noto Serif\", \"DejaVu Serif\", \"Liberation Serif\", Georgia, serif",
        _ => "\"Adwaita Sans\", \"Inter\", \"Cantarell\", \"Noto Sans\", system-ui, sans-serif",
    };
    let n = |x: f64| format!("{:.3}", x.clamp(0.0, 1.0));
    let mut out = String::with_capacity(BASE.len() + GLASS.len() + 512);
    out.push_str(BASE);
    out.push_str(GLASS);
    // the panel colour, a little see-through for the glass look
    let glasspanel = glass_panel(&v.panel2);
    out.replace("@bg@", &v.bg)
        .replace("@acc@", &v.acc)
        .replace("@ink@", &v.ink)
        .replace("@panel@", &solid_panel(&v.panel))
        .replace("@panel2@", &v.panel2)
        .replace("@glasspanel@", &glasspanel)
        .replace("@glassh@", &n(g + 0.04))
        .replace("@glass@", &n(g))
        .replace("@g23@", &n(g * 2.3))
        .replace("@g28@", &n(g * 2.8))
        .replace("@g06@", &n(g * 0.6))
        .replace("@g08@", &n(g * 0.8))
        .replace("@g13@", &n(g * 1.3))
        .replace("@g16@", &n(g * 1.6))
        .replace("@font@", font)
}

/// The popup colour, a little more opaque than the original (GTK can't blur what's behind it).
fn solid_panel(rgba: &str) -> String {
    match rgba.rsplit_once(',') {
        Some((head, _)) if rgba.starts_with("rgba(") => format!("{head},.985)"),
        _ => rgba.to_string(),
    }
}

fn glass_panel(hex: &str) -> String {
    let (r, g, b) = crate::config::hex_rgb(hex);
    format!("rgba({},{},{},.93)", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// CSS classes on the window for the look's switches.
pub fn look_classes(look: &Look) -> Vec<(&'static str, bool)> {
    vec![
        ("glass", look.glass()),
        ("sharp", look.corners == "sharp"),
        ("compact", look.button_size == "compact"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn every_placeholder_is_filled() {
        let mut l = Settings::default().look();
        for (theme, style) in [("aurora", "classic"), ("oled", "glass")] {
            l.theme = theme.into();
            l.style = style.into();
            let c = css(&l);
            assert!(!c.contains('@'), "unfilled placeholder");
            assert!(c.contains("#34e0a1"));
        }
    }
}
