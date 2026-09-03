#!/usr/bin/env python3
"""本家の試験の入力で、往復(読んで書き戻す)の合否を本家の HTML で見る。

    ruby tools/honke_record.rb vendor/asciidoctor/test/lists_test.rb lists.json
    python3 tools/honke_blocks.py lists.json [--show N] [--bin 径路]

1つの入力につき、本家で組んだ HTML(元)と、うちで往復させた字を本家で組んだ
HTML を比べる。`<pre>` の外では空白の並びを1つと見なす(第3歩の比べ方)。
opts の付いた呼び出し(属性や backend の指定)は数えない。
"""
import json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUBY = r'''
require 'json'
$LOAD_PATH.unshift File.join(ARGV[0], 'vendor/asciidoctor/lib')
require 'asciidoctor'
items = JSON.parse(STDIN.read)
out = items.map do |it|
  h = it.map { |k, v| [k, (v.nil? ? nil : (begin; Asciidoctor.convert(v, safe: :safe, standalone: false); rescue => e; "ERR #{e.class}: #{e.message}"; end))] }.to_h
  h
end
puts JSON.generate(out)
'''

def normalize(h):
    # <pre>…</pre> の中は触らず、外の空白の並びを1つに
    parts = re.split(r'(<pre[\s\S]*?</pre>)', h)
    return ''.join(p if p.startswith('<pre') else re.sub(r'\s+', ' ', p).strip() for p in parts)

def main():
    args = sys.argv[1:]
    show = 0
    binp = os.environ.get('ROUNDTRIP_BIN', os.path.join(ROOT, 'target', 'release', 'examples', 'roundtrip'))
    if '--show' in args:
        show = int(args[args.index('--show') + 1])
    if '--bin' in args:
        binp = args[args.index('--bin') + 1]
    if '--dir' in args:
        # 本家の .adoc を全部(176 枚)。試験の記録と同じ比べ方で数える
        root = args[args.index('--dir') + 1]
        rec = []
        for dp, _, fs in os.walk(root):
            for f in sorted(fs):
                if f.endswith('.adoc'):
                    path = os.path.join(dp, f)
                    try:
                        rec.append({'test': os.path.relpath(path, root), 'src': open(path, encoding='utf-8').read(), 'opts': []})
                    except UnicodeDecodeError:
                        pass
        src_json = os.path.basename(root.rstrip('/')) + '/'
    else:
        src_json = [a for a in args if a.endswith('.json')][0]
        rec = json.load(open(src_json))
    seen, cases = set(), []
    for r in rec:
        if r['opts'] or r['src'] in seen:
            continue
        seen.add(r['src']); cases.append(r)
    outs, fails = [], []
    for r in cases:
        p = subprocess.run([binp], input=r['src'].encode(), capture_output=True)
        if p.returncode != 0:
            fails.append((r['test'], p.stderr.decode().strip()[:80])); outs.append(None)
        else:
            outs.append(p.stdout.decode())
    pairs = [{'a': r['src'], 'b': o} for r, o in zip(cases, outs)]
    html = json.loads(subprocess.run(['ruby', '-e', RUBY, ROOT], input=json.dumps(pairs).encode(), capture_output=True, check=True).stdout)
    same = byte = 0
    diffs = []
    for r, o, h in zip(cases, outs, html):
        if o is None:
            continue
        if o == r['src']:
            byte += 1
        if h['b'] is not None and normalize(h['a']) == normalize(h['b']):
            same += 1
        else:
            diffs.append((r['test'], r['src'], o))
    n = len(cases)
    print(f"{os.path.basename(src_json) or src_json}: 入力 {n} / 読めない {len(fails)} / 1バイトも変わらない {byte} / 本家の HTML が同じ {same} / 違う {len(diffs)}")
    for t, e in fails[:show]:
        print(f"  読めない {t}: {e}")
    brief = '--brief' in args
    for t, a, b in (diffs if brief else diffs[:show]):
        if brief:
            la, lb = a.split('\n'), b.split('\n')
            i = next((k for k in range(max(len(la), len(lb))) if k >= len(la) or k >= len(lb) or la[k] != lb[k]), 0)
            x = la[i] if i < len(la) else '<無>'
            y = lb[i] if i < len(lb) else '<無>'
            print(f"  {t[:60]:60} | {x[:40]!r} -> {y[:40]!r}")
        else:
            print(f"  --- {t}\n  元:  {a!r}\n  往復: {b!r}")

if __name__ == '__main__':
    main()
