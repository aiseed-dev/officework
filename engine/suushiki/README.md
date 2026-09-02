# 数式を組むための同梱物

`engine/src/suushiki.rs` が埋め込みます。

- `NewCMMath-Book.otf` — 数式の書体(New Computer Modern Math)。
  GUST Font License(`NewCMMath-LICENSE.txt`)。typst-assets 0.15.1 から写しました。
  本文の書体は同梱しない決まり(.gitignore の `assets/`)ですが、数式の書体は
  機械に無いのが普通なので、この1つだけ同梱します(2026-09-02)
- `mitex/` — mitex(LaTeX → typst)の typst 側の定義。Apache-2.0(`mitex/LICENSE`)。
  https://github.com/mitex-rs/mitex の packages/mitex/specs から写しました。
  `\sqrt` `\text{}` 行列などが、この定義に頼ります
