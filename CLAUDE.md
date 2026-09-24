# Claude への決まり

どの AI にも共通の決まりは AGENTS.md にあります。先に読みます。

@AGENTS.md

ここには Claude Code だけに関わることを書きます。

## 作業の場所

- セッションが `.claude/worktrees/…` の worktree で始まっても、作業は
  `~/dev/officework` で直接行います(AGENTS.md「作業の進め方」)。
- Write・Edit のツールが worktree の外のファイルを断るときは、シェル
  (python や heredoc)で `~/dev/officework` のファイルを書きます。
- シェルの作業フォルダーは毎回 worktree に戻ります。コマンドは
  `cd ~/dev/officework && …` で始めます。

## 長い処理の待ち方

- アプリのビルドや長いテストは `run_in_background` で走らせ、終わりの知らせを待ちます。
- `pgrep -f` で待つループは書きません。ループのコマンド行が自分自身に一致して、
  いつまでも終わりません(2026-09-24 に 8 個残りました)。

## メモリーとの分け方

- 発注者と決めたことで、他の AI にも要るものは AGENTS.md に書きます。
- Claude の働き方だけに関わること(返答の仕方、Opus への渡し方など)はメモリーに書きます。
