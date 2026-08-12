# conda-forge へ出す準備(2026-08-12 起こし)

pyproject の頭にある方針(2026-08-09 発注者「PyPI と conda-forge に公開する
ところから始めましょう」)の conda-forge 側。**提出先はこの repo ではなく
[conda-forge/staged-recipes](https://github.com/conda-forge/staged-recipes)** —
ここにあるのは下書き(recipe/meta.yaml)と検証の控え。

## 検証済みのこと(2026-08-12、この機械で実測)

- `maturin sdist -m pysheet/Cargo.toml` が**自己完結の sdist を作る** —
  path 依存(sheet 27・engine 11・ooxml 9 ファイル)を同梱し、
  取り出した木だけで cargo が解ける。conda-forge はこの sdist から組む
- publish-pypi.yml は既に sdist を PyPI へ上げている
  (「元の形(sdist)。conda-forge はこちらを見る」の段)

## 手順(公開の版が PyPI に上がってから)

1. **sha256 を取る**(meta.yaml の頭に取り方のコマンド)
2. meta.yaml の `version` と `sha256` を差し替える
3. staged-recipes を fork し、`recipes/officework/meta.yaml` に置いて PR
4. CI(linux-64 / osx-64 / osx-arm64)が通るのを見る。**落ちたら
   このファイルに踏んだ穴を書き足す**
5. 取り込まれると feedstock(conda-forge/officework-feedstock)ができ、
   以後の版上げは feedstock 側の bot PR(sha256 差し替え)になる

## 決めてあること

- **Windows は最初は skip**(`skip: true  # [win]`)。まず Linux と mac
- **AGPL の表示義務**: Rust 依存の免許は `cargo-bundle-licenses` で
  THIRDPARTY.yml に束ねて license_file に載せる(conda-forge の作法)
- run 依存は python だけ(pandas は optional のまま — conda では
  variant を作らず、使う人が別途入れる)

## 未確認(提出前に確かめる)

- **abi3 の扱い**: wheel は cp310-abi3 の1枚だが、conda-forge の既定は
  **Python の版ごとに組む**。abi3 を1つの build で済ませる仕組み
  (`python_version_independent` / python-abi3)は比較的新しく、
  staged-recipes の審査での通り方が未確認 — **最初は版ごとビルドの
  素直な形で出し、feedstock 化の後に abi3 化を計る**のが安全
- `{{ compiler('rust') }}` と `cargo-bundle-licenses` の最新の書き方は
  staged-recipes の実例(最近取り込まれた maturin 物)を1つ写して確かめる
- ネット遮断ビルドで cargo が crates.io を引けない環境向けの
  `cargo vendor` が要るか(conda-forge の CI は基本ネット可なので
  多分不要だが、審査員に言われたら vendor に切り替える)

## この repo 側でやっておくこと(残件)

- [ ] 公開の版(v0.2.0 予定)のタグ → PyPI(先に workflow_dispatch の
      「wheel を作るだけ」で予行)
- [ ] `extra.recipe-maintainers` を発注者の GitHub ID に(いまは仮に
      aiseed-dev)
