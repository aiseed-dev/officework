"""Measure the space Word leaves between two paragraphs.

    .venv/bin/python tools/para_spacing.py <docx>...

Each docx needs a Word PDF beside it (`<name>.ms.pdf`). The script walks the
body paragraphs, keeps the adjacent pairs whose line boxes are the same size
(same style, or the same line rule and factor), finds their lines in Word's
PDF and prints the space Word left between them next to what the file asks
for. The line height comes from a paragraph of that style with two or more
lines, so no font metric is needed.

Word lays the page out at 1/300 inch, so every measurement is a multiple of
0.24pt; read a difference under that as a match.

Pairs with `w:contextualSpacing`, a line break, or automatic spacing
(`w:beforeAutospacing`) are left out, because those decide the space by
another rule.
"""
import sys, os, zipfile, re, statistics
from xml.etree import ElementTree as ET
import pdfplumber
W='{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'

def read_sp(sp,e):
    for k,a in (('before','before'),('after','after')):
        if sp.get(W+a) is not None: e[k]=int(sp.get(W+a))/20
    if sp.get(W+'line') is not None: e['line']=int(sp.get(W+'line'))
    if sp.get(W+'lineRule') is not None: e['rule']=sp.get(W+'lineRule')
    for k in ('beforeAutospacing','afterAutospacing'):
        if sp.get(W+k) in ('1','true'): e[k]=True

def styles_of(z):
    root=ET.fromstring(z.read('word/styles.xml'))
    dflt=dict(before=0.0,after=0.0,line=None,rule=None,ctx=False,
              beforeAutospacing=False,afterAutospacing=False)
    dd=root.find(W+'docDefaults')
    if dd is not None:
        sp=dd.find(W+'pPrDefault/'+W+'pPr/'+W+'spacing')
        if sp is not None: read_sp(sp,dflt)
    d={}
    for st in root.findall(W+'style'):
        if st.get(W+'type')!='paragraph': continue
        e=dict(basedOn=None,before=None,after=None,line=None,rule=None,ctx=None,
               beforeAutospacing=None,afterAutospacing=None)
        b=st.find(W+'basedOn')
        if b is not None: e['basedOn']=b.get(W+'val')
        sp=st.find(W+'pPr/'+W+'spacing')
        if sp is not None: read_sp(sp,e)
        if st.find(W+'pPr/'+W+'contextualSpacing') is not None: e['ctx']=True
        d[st.get(W+'styleId')]=e
    return d,dflt

def resolve(sid,styles,dflt):
    chain=[]; s=sid
    while s in styles and s not in chain:
        chain.append(s); s=styles[s]['basedOn']
    out=dict(dflt)
    for s in reversed(chain):
        for k,v in styles[s].items():
            if k!='basedOn' and v is not None: out[k]=v
    return out

def paras_of(path):
    z=zipfile.ZipFile(path)
    styles,dflt=styles_of(z)
    body=ET.fromstring(z.read('word/document.xml')).find(W+'body')
    out=[]
    for p in list(body):
        if p.tag!=W+'p':
            out.append(None); continue
        ppr=p.find(W+'pPr'); sid=None
        own=dict(before=None,after=None,line=None,rule=None,ctx=None,
                 beforeAutospacing=None,afterAutospacing=None)
        if ppr is not None:
            ps=ppr.find(W+'pStyle')
            if ps is not None: sid=ps.get(W+'val')
            sp=ppr.find(W+'spacing')
            if sp is not None: read_sp(sp,own)
            if ppr.find(W+'contextualSpacing') is not None: own['ctx']=True
        eff=resolve(sid or 'Normal',styles,dflt)
        for k,v in own.items():
            if v is not None: eff[k]=v
        txt=''.join(t.text or '' for t in p.iter(W+'t'))
        brk = p.find('.//'+W+'br') is not None or ppr is not None and ppr.find(W+'sectPr') is not None
        out.append((sid,eff,txt,brk))
    return out

def pdf_lines(path):
    out=[]
    with pdfplumber.open(path) as d:
        for pi,page in enumerate(d.pages,1):
            rows=[]
            for c in sorted(page.chars,key=lambda c:(-c["matrix"][5],c["x0"])):
                if c["text"].isspace(): continue
                base=page.height-c["matrix"][5]
                for r in rows:
                    if abs(r["top"]-base)<=2.0: r["chars"].append(c); break
                else: rows.append({"top":base,"chars":[c]})
            for r in sorted(rows,key=lambda r:r["top"]):
                cs=sorted(r["chars"],key=lambda c:c["x0"])
                out.append((pi,r["top"],"".join(c["text"] for c in cs)))
    return out

def norm(s):
    return re.sub(r'\s+','',s).replace('’',"'").replace('‘',"'").lower()

def find(lines,text):
    """Return the run of PDF lines that spells out `text` (page, [baselines])."""
    t=norm(text)
    if len(t)<8: return None
    hits=[]
    for i,(pi,y,s) in enumerate(lines):
        s=norm(s)
        if not s or not t.startswith(s): continue
        j=i; got=''
        ys=[]
        while j<len(lines) and lines[j][0]==pi and t.startswith(got+norm(lines[j][2])):
            got+=norm(lines[j][2]); ys.append(lines[j][1]); j+=1
            if got==t: break
        if got==t: hits.append((pi,ys))
    return hits[0] if len(hits)==1 else None

def scan(docx,pdf):
    ps=paras_of(docx)
    lines=pdf_lines(pdf)
    # line height per style, from any paragraph of that style with 2+ lines
    box={}
    found={}
    for k,p in enumerate(ps):
        if not p or not p[2].strip() or p[3]: continue
        f=find(lines,p[2])
        if not f: continue
        found[k]=f
        if len(f[1])>=2:
            d=[b-a for a,b in zip(f[1],f[1][1:])]
            box.setdefault(p[0],[]).extend(d)
    out=[]
    for k in range(len(ps)-1):
        a,b=ps[k],ps[k+1]
        if not a or not b: continue
        same = a[0]==b[0] or (a[1]['line']==b[1]['line'] and a[1]['rule']==b[1]['rule'])
        if not same: continue
        if a[1]['ctx'] or a[3] or b[3]: continue
        if a[1].get('afterAutospacing') or b[1].get('beforeAutospacing'): continue
        if k not in found or k+1 not in found: continue
        if found[k][0]!=found[k+1][0]: continue   # same page
        h=box.get(a[0]) or box.get(b[0])
        if not h: continue
        lh=statistics.median(h)
        gap=found[k+1][1][0]-found[k][1][-1]
        out.append((a[0], a[1]['after'], b[1]['before'], round(gap-lh,2)))
    return out

for docx in sys.argv[1:]:
    pdf=docx[:-5]+'.ms.pdf'
    if not os.path.exists(pdf): continue
    try: r=scan(docx,pdf)
    except Exception as e:
        print(os.path.basename(docx),'!!',e); continue
    for style,af,bf,s in r:
        if af==bf==0: continue
        print(f"{os.path.basename(docx)[:8]} {str(style)[:14]:14} after={af:5.1f} before={bf:5.1f} measured={s:6.2f} sum={af+bf:5.1f} max={max(af,bf):5.1f}")
