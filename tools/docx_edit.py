"""
Helpers for editing board_User_Manual_Android.docx in place, keeping its styles.

Usage (python3 -I, python-docx installed):
    import docx, sys; sys.path.insert(0, "tools"); from docx_edit import *
    d = docx.Document("board_User_Manual_Android.docx"); P = list(d.paragraphs); T = list(d.tables)
    init(P)
    setp(find("Status pill:"), "**Status pill:** ...")          # **bold** segments
    x = after(find("Some paragraph"), "New paragraph", like=find("Home screen widget that you can resize"))  # copies a bullet's style
    addrow(T[9], ["What you see", "Likely cause", "Fix"], at=3)
    d.save("board_User_Manual_Android.docx")
"""
import copy, re
import docx
import docx.table
import docx.text.paragraph

def segs(md):
    parts = re.split(r'(\*\*[^*]+\*\*)', md)
    return [(p[2:-2], True) if p.startswith('**') else (p, False) for p in parts if p]

def setp(p, md):
    runs = p.runs
    base = runs[0]._r if runs else None
    # pick a non-bold run as the template if there is one
    for r in runs:
        if not r.bold: base = r._r; break
    for r in list(p.runs): r._r.getparent().remove(r._r)
    for text, bold in segs(md):
        r = p.add_run(text)
        if base is not None and base.rPr is not None:
            r._r.insert(0, copy.deepcopy(base.rPr))
        r.bold = True if bold else None
    return p

def after(p, md, like=None):
    src_p = like if like is not None else p
    new = copy.deepcopy(src_p._p)
    p._p.addnext(new)
    np = docx.text.paragraph.Paragraph(new, p._parent)
    return setp(np, md)

def before(p, md, like):
    new = copy.deepcopy(like._p)
    p._p.addprevious(new)
    np = docx.text.paragraph.Paragraph(new, p._parent)
    return setp(np, md)

def cell(c, md):
    setp(c.paragraphs[0], md)
    for extra in c.paragraphs[1:]: extra._p.getparent().remove(extra._p)

def addrow(t, vals, like_idx=-1, at=None):
    row = copy.deepcopy(t.rows[like_idx]._tr)
    if at is None: t._tbl.append(row)
    else: t.rows[at]._tr.addprevious(row)
    r = docx.table._Row(row, t)
    for c, v in zip(r.cells, vals): cell(c, v)
    return r

def delrow(t, first_text):
    for r in t.rows:
        if r.cells[0].text.strip() == first_text:
            r._tr.getparent().remove(r._tr); return
    raise SystemExit('row not found: ' + first_text)

P = []

def init(paragraphs):
    global P
    P[:] = list(paragraphs)

def find(prefix):
    for p in P:
        if p.text.startswith(prefix): return p
    raise SystemExit('not found: ' + prefix)

