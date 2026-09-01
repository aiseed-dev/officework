#!/usr/bin/env python3
"""OOXML の図形の定義(vendor/ooxml-shapes)から book/src/preset_gen.rs を生成する。

定義データには、形ごとに 調整値の既定(avLst)・座標の計算式(gdLst)・
線の引き方(pathLst)が入っている。ここでは意味の解釈はせず、
Rust の静的な表に写すだけ。式と座標の解釈は book/src/preset_spec.rs が
実行時に行う。

使い方: python3 tools/gen_preset_shapes.py
"""

import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "vendor/ooxml-shapes/presetShapeDefinitions.xml"
DST = ROOT / "book/src/preset_gen.rs"
NS = "{http://schemas.openxmlformats.org/drawingml/2006/main}"


def local(tag: str) -> str:
    return tag.split("}")[-1]


def path_cmds(p) -> str:
    """path の子要素を「命令1字 + 引数」の字句の列にする。"""
    toks = []
    for c in p:
        t = local(c.tag)
        if t == "moveTo":
            pt = c.find(NS + "pt")
            toks += ["M", pt.get("x"), pt.get("y")]
        elif t == "lnTo":
            pt = c.find(NS + "pt")
            toks += ["L", pt.get("x"), pt.get("y")]
        elif t == "arcTo":
            toks += ["A", c.get("wR"), c.get("hR"), c.get("stAng"), c.get("swAng")]
        elif t == "cubicBezTo":
            pts = c.findall(NS + "pt")
            toks += ["C"]
            for pt in pts:
                toks += [pt.get("x"), pt.get("y")]
        elif t == "quadBezTo":
            pts = c.findall(NS + "pt")
            toks += ["Q"]
            for pt in pts:
                toks += [pt.get("x"), pt.get("y")]
        elif t == "close":
            toks += ["Z"]
        else:
            raise SystemExit(f"知らない命令: {t}")
    return " ".join(toks)


def main() -> None:
    root = ET.parse(SRC).getroot()
    out = []
    out.append("//! OOXML の図形の定義(187種)。**生成物 — 手で直さない。**")
    out.append("//!")
    out.append("//! 元は vendor/ooxml-shapes/presetShapeDefinitions.xml で、")
    out.append("//! `python3 tools/gen_preset_shapes.py` が作り直す。")
    out.append("//! 式(`gd`)と命令(`cmds`)の解釈は [`crate::preset_spec`]。")
    out.append("")
    out.append("/// 1つの形の定義")
    out.append("pub struct SpecShape {")
    out.append("    pub name: &'static str,")
    out.append('    /// 調整値の既定(avLst)。式は必ず "val N" なので値だけ持つ')
    out.append("    pub adj: &'static [(&'static str, f32)],")
    out.append('    /// 計算式(gdLst)。式は "演算 引数…" の前置きの字句')
    out.append("    pub gd: &'static [(&'static str, &'static str)],")
    out.append("    pub paths: &'static [SpecPath],")
    out.append("}")
    out.append("")
    out.append("/// 線の引き方(pathLst の1本)")
    out.append("pub struct SpecPath {")
    out.append("    /// この path の座標系の幅と高さ(0 なら実寸のまま)")
    out.append("    pub w: f32,")
    out.append("    pub h: f32,")
    out.append("    pub fill: bool,")
    out.append("    pub stroke: bool,")
    out.append('    /// 命令の字句: "M x y" "L x y" "A wR hR stAng swAng"')
    out.append('    /// "C x1 y1 x2 y2 x y" "Q x1 y1 x y" "Z" を空白でつなぐ')
    out.append("    pub cmds: &'static str,")
    out.append("}")
    out.append("")
    out.append("pub static SHAPES: &[SpecShape] = &[")
    n = 0
    for shape in root:
        name = local(shape.tag)
        adj = []
        av = shape.find(NS + "avLst")
        if av is not None:
            for gd in av.findall(NS + "gd"):
                f = gd.get("fmla").split()
                if f[0] != "val":
                    raise SystemExit(f"{name}: avLst に val 以外: {f}")
                adj.append((gd.get("name"), float(f[1])))
        gds = []
        gl = shape.find(NS + "gdLst")
        if gl is not None:
            for gd in gl.findall(NS + "gd"):
                gds.append((gd.get("name"), gd.get("fmla")))
        paths = []
        for p in shape.iter(NS + "path"):
            paths.append(
                (
                    float(p.get("w", 0)),
                    float(p.get("h", 0)),
                    p.get("fill", "norm") != "none",
                    p.get("stroke", "true") != "false",
                    path_cmds(p),
                )
            )
        out.append("    SpecShape {")
        out.append(f'        name: "{name}",')
        a = ", ".join(f'("{k}", {v:.1f})' for k, v in adj)
        out.append(f"        adj: &[{a}],")
        g = ", ".join(f'("{k}", "{v}")' for k, v in gds)
        out.append(f"        gd: &[{g}],")
        out.append("        paths: &[")
        for w, h, fill, stroke, cmds in paths:
            out.append(
                f"            SpecPath {{ w: {w:.1f}, h: {h:.1f}, "
                f"fill: {str(fill).lower()}, stroke: {str(stroke).lower()}, "
                f'cmds: "{cmds}" }},'
            )
        out.append("        ],")
        out.append("    },")
        n += 1
    out.append("];")
    out.append("")
    DST.write_text("\n".join(out), encoding="utf-8")
    print(f"{DST.relative_to(ROOT)}: {n} 形")


if __name__ == "__main__":
    main()
