# presetShapeDefinitions.xml

OOXML(ECMA-376)の DrawingML が定める 187 種の図形
(オートシェイプ)の定義データです。形ごとに、調整値の既定
(avLst)・座標の計算式(gdLst)・線の引き方(pathLst)が
書いてあります。

- 出どころ: LibreOffice の写し
  https://raw.githubusercontent.com/LibreOffice/core/master/oox/source/drawingml/customshapes/presetShapeDefinitions.xml
  (2026-09-01 取得)
- 元の定義: ECMA-376 Part 1 (Office Open XML) の付属データ
- ライセンス: LibreOffice は MPL-2.0。MPL-2.0 は AGPL への
  組み込みを許しています(Larger Work の条項)

`tools/gen_preset_shapes.py` がこれを読んで
`book/src/preset_gen.rs` を生成します。このファイルを手で
直さないでください — 直すのは生成スクリプトの側です。
