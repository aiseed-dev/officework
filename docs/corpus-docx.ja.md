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
