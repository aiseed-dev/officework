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

**形式は v1(recipe.yaml)が必須**(2026-08-14 に実測 — staged-recipes の
README が「v0 の meta.yaml は新規には非推奨」と明言。下書きは v1 に
作り直し済み)。

1. **sha256 を取る**(recipe.yaml の頭に取り方のコマンド)
2. recipe.yaml の `context.version` と `sha256` を差し替える。
   `extra.recipe-maintainers` を**発注者の個人の GitHub ID** に
   (組織名は不可。PR に「maintainer になる」と一言コメントする決まり)
3. [staged-recipes](https://github.com/conda-forge/staged-recipes) を
   fork し、`recipes/officework/recipe.yaml` に置いて main へ PR
4. CI(linux-64 / osx-64 / osx-arm64)が通るのを見る。**落ちたら
   このファイルに踏んだ穴を書き足す**。審査は人間のボランティア —
   数日〜数週かかることがある。急かすなら PR に
   `@conda-forge/help-python-c` を1回だけ
5. 取り込まれると feedstock(conda-forge/officework-feedstock)が
   自動で出来て、数時間で `conda install -c conda-forge officework` が
   通るようになる。以後の版上げは PyPI に上げるたび bot
   (regro-cf-autotick-bot)が feedstock に sha256 差し替えの PR を
   出してくるので、maintainer が merge するだけ

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

- [x] 公開の版のタグ → PyPI(0.2.0 は 2026-08-12 に済)
- [x] v0.3.0 のタグ → PyPI(2026-08-14 に済。recipe は 0.3.0 の sha256 入り)
- [x] `extra.recipe-maintainers` = awoni(発注者の個人アカウント。2026-08-14)
- [ ] staged-recipes へ PR(下の「出し方」— recipe.yaml はそのまま写せる形にした)
- [x] recipe を v1(recipe.yaml)に(2026-08-14。v0 の meta.yaml は消した)

## 出し方(そのまま打てる形。fork と PR は awoni のアカウントで)

```console
$ gh repo fork conda-forge/staged-recipes --clone
$ cd staged-recipes
$ git switch -c officework
$ mkdir recipes/officework
$ cp /home/dev/dev/officework/packaging/conda/recipe/recipe.yaml recipes/officework/
$ git add recipes/officework && git commit -m "Add officework"
$ git push -u origin officework
$ gh pr create --repo conda-forge/staged-recipes --title "Add officework" \
    --body "xlsx/docx engines with formula recalculation (Rust + maturin, abi3). Built from the PyPI sdist; licenses of Rust dependencies are bundled via cargo-bundle-licenses."
```

PR を出した後の作法(テンプレートから 2026-08-14 に採取):

- **審査は頼まないと来ない。** CI が緑になったら PR に
  `@conda-forge/help-python-c, ready for review!` とコメントする
  (Rust + maturin の Python 拡張は python/c hybrid の班。rust の班もあるが、
  Python パッケージとしての審査はこちら)
- **初投稿の人は班を直接呼べない**(GitHub の制限)。その場合は
  `@conda-forge-admin, please ping conda-forge/help-python-c` と
  コメントすると bot が代わりに呼ぶ
- 審査は人間のボランティアで**数日〜数週**。急かさず、直しの指摘には
  こまめに応じる。CI が落ちたら**このファイルに踏んだ穴を書き足してから**直す

maintainer は awoni。PR の作者が awoni 本人なら同意のコメントは不要
(他人が出す PR に名を載せるときだけ「I agree to be a maintainer」が要る)。

## 踏んだ穴

- **0.3.0 の sdist に LICENSE が入っていない**(2026-08-14、PR のチェック
  リストを検分していて発見 — CI より先に見つかった)。license_file が
  見つからずビルドが落ちる形。recipe は**第二の source** でタグの LICENSE を
  取り、根に置いて解決(sha256 で釘付け)。リポジトリ側は pysheet/LICENSE を
  置いたので **0.4.0 からは sdist が `pysheet/LICENSE` を運ぶ** — その版から
  第二の source を消し、`license_file: pysheet/LICENSE` に切り替えられる
