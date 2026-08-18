# 他人が書いた docx の目録(pydoc_diff の突き合わせ用)

`docs/corpus.ja.md`(xlsx)の作法をそのまま docx に当てた物。
**現物はこの repo に置かない。** 置き場は `~/docx-corpus/`(既定)。
ここに残すのは**目録**(出所・大きさ・sha256 の頭・何を試すための1枚か)だけ。

理由は xlsx と同じ二つ — **免許**(他人の docx を Apache/AGPL の木に入れると
配れなくなる)と**個人情報**(実物の書類には人名が入る。repo に入れたら消せない)。

**受入試験はこの目録に依らせない。** corpus は取り直す物なので、
試験がそれに依ると CI で回らない。数式の試験(`ooxml` の `anchor_tests`)は
**下の現物から写した形を手で組んである** — 現物そのものは要らない。

## 第1便(2026-08-10。2枚。数式)

| ファイル | 大きさ | sha256 の頭 | 出所 |
|---|---|---|---|
| pandoc-math.docx | 10,385 | `af489654fc52e94e` | 下の作り方(pandoc 3.1.3) |
| libreoffice-math.docx | 7,497 | `aa1bf19c5c7d378c` | 下の作り方(LibreOffice 24.2.7.2) |

### 作り方(取り直す)

    mkdir -p ~/docx-corpus && cd ~/docx-corpus
    cat > src.md <<'EOF'
    # 数式の見本

    インラインの数式 $a^2 + b^2 = c^2$ を含む段落です。

    独立した数式:

    $$\int_{0}^{\infty} e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}$$

    分数と根号: $\frac{-b \pm \sqrt{b^2-4ac}}{2a}$

    行列:

    $$\begin{pmatrix} a & b \\ c & d \end{pmatrix}$$
    EOF
    # 1枚目 — pandoc が直に書く OMML
    pandoc src.md -o pandoc-math.docx
    # 2枚目 — それを LibreOffice に通し、**向こうの書き手**に書き直させる
    mkdir -p t && cp pandoc-math.docx t/conv.docx
    soffice --headless --convert-to odt  --outdir t  t/conv.docx
    soffice --headless --convert-to 'docx:MS Word 2007 XML' --outdir t2 t/conv.odt
    cp t2/conv.docx libreoffice-math.docx && rm -rf t t2

### **この2枚の限界を先に書いておく**

**どちらも手元で作った物で、世の中で拾った文書ではない。** 元は同じ src.md で、
違うのは**書き手**(pandoc と LibreOffice Writer)だけ。
Word 本体と Google ドキュメントの書き出しはまだ無い。

それでも1枚だけよりはるかに役に立った — **2つの書き手は
名前空間の書き方が正反対で、そこに穴が2つあった**(下)。
Word 本体の1枚が入れば、また別の癖が出る見込み。**次に足すならそこ。**

## この2枚で出た穴(2026-08-10)

数式を原文で持ち越す仕事(`carry_math`)で、**自分で書いた文書では
永久に出ない形**が2つ出た。どちらも「書き手が違う」ことでしか見つからない:

1. **`xmlns:m` の置き場が書き手で違う。**
   pandoc は root の `<w:document>` に宣言し、`<m:oMath>` は裸で書く。
   LibreOffice は root に宣言せず、**`<m:oMath xmlns:m="…">` と要素側に**書く。
   持ち越すときに宣言を足す作りだと、後者では属性が二重になって XML が壊れる
2. **`xml:space="preserve"`。** LibreOffice の `<m:t>` はこれを書く。
   `xml:` は XML の定めで最初から結びついていて宣言してはいけないのに、
   「解決できない接頭辞」と数えていたので、**数式が4つとも丸ごと落ちていた**

## 通し方

    cd ~/dev/officework
    export PATH="$HOME/.local/node/bin:$PATH"          # node 24
    export GENOFFICE=~/dev/genoffice                   # 向こうの木(tsx が要る)
    OFFICEWORK_PYTHON=<.so を置いた木の python> \
      python3 tools/pydoc_diff.py --roundtrip --corpus ~/docx-corpus

## 第2便(2026-08-10。4枚。節と脚注)

| ファイル | 大きさ | sha256 の頭 | 出所 |
|---|---|---|---|
| lo-twosect.docx | 5,137 | `884ffd2e71aa3a91` | 下の作り方(LibreOffice。縦→横の2節) |
| pandoc-fn.docx | 10,243 | `c855f916c1f82f15` | 下の作り方(pandoc。脚注3つ) |
| lo-fn.docx | 7,963 | `97441e900041afc0` | 上を LibreOffice に通したもの |
| both-notes.docx | 6,683 | `c9f570c57fa5a638` | 下の作り方(LibreOffice。**脚注と文末脚注が同居**) |

### `both-notes.docx` は**穴を塞ぐために作った1枚**

脚注を紙面に出す仕事を入れた直後、注を **id だけで引いていた**のを見つけた。
docx は `footnotes.xml` と `endnotes.xml` を**別々に番号付けする**ので、
両方を含む文書では **id が必ず衝突する**。この1枚の実物がまさにそれで:

    本文の印        脚注 id=2 / 文末 id=2 / 脚注 id=3 / 文末 id=3
    footnotes.xml   0 1 2 3
    endnotes.xml    0 1 2 3      ← 同じ番号

**集めていた2枚に文末脚注が入っていなかったので、試験も突き合わせも
緑のままだった。** 直す前のコードにこの1枚を通すと、文末脚注の印に
**脚注の文章**が出る(確認済み)。SEKKEI.adoc の
「緑は『正しい』ではなく『この物差しでは差が出ない』」がそのまま当たる。

### 作り方(取り直す)

節・脚注・両方入りは、**flat ODF(.fodt)を書いて LibreOffice に通す**。
pandoc は文末脚注を書き出せず、LibreOffice も docx を素通しすると
節を落とすことがあるので、この経路がいちばん確実だった。

    # 脚注(pandoc → LibreOffice の2枚)
    pandoc fn-src.md -o pandoc-fn.docx        # 本文に [^a] 形式の脚注
    mkdir -p t && cp pandoc-fn.docx t/c.docx
    soffice --headless --convert-to odt --outdir t t/c.docx
    soffice --headless --convert-to 'docx:MS Word 2007 XML' --outdir t2 t/c.odt
    cp t2/c.docx lo-fn.docx && rm -rf t t2

    # 節(縦→横)と、脚注+文末脚注の同居は .fodt から
    soffice --headless --convert-to 'docx:MS Word 2007 XML' --outdir . lo-twosect.fodt
    soffice --headless --convert-to 'docx:MS Word 2007 XML' --outdir . both-notes.fodt

`.fodt` の原本も同じ置き場に残してある(`lo-twosect.fodt`・`both-notes.fodt`)。
**節の切り替えは、段落様式に `style:master-page-name` を付けた
「名前付きの様式」でないと LibreOffice が拾わない** — 自動様式に付けても
落ちる。ここで一度空振りしている。

## 第3便(2026-08-13。1枚。三つが重なる形)

| ファイル | 大きさ | sha256 の頭 | 出所 |
|---|---|---|---|
| all3.docx | 6,744 | `34d62c370ec79a39` | 下の作り方(LibreOffice) |

**数式・節・脚注を別々の回で作ったので、一緒に動かしたことが無かった。**
継ぎ目を見るための1枚:

- 節が2つ(**縦 → 横**)
- **それぞれの節に脚注**(ここが継ぎ目 — 脚注の高さは本文の底を上げ、
  節ごとに紙の高さが違う。どちらも `paginate_full` の同じ輪の中)
- 文末脚注も1つ(置き場が違うことの確認)

    soffice --headless --convert-to 'docx:MS Word 2007 XML' --outdir . all3.fodt

通した結果(2026-08-13): 保存は節 2→2・脚注 2→2・文末 1→1・部品の欠け無し。
組版は**頁ごとに紙が変わり、脚注はそれぞれ自分の頁の下**に出た
(縦の頁は余白 20mm、横の頁は 30mm で別々に効いている)。
PDF は 1頁目 A4 縦・2頁目 A4 横。**直す所は無かった。**

### 次に足すなら

- **Word 本体**の書き出し(まだ1枚も無い。名前空間の癖が3つ目として出る見込み)
- **Google ドキュメント**の書き出し
- 節が3つ以上で**途中だけ向きが違う**もの(いまは2節どまり)
