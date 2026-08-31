# 報告書.docx の字句を差し替える — エンジンだけの見本(アプリ不要)。
#
#   pip install officework
#   python3 sample/差し替え.py
#
# python-docx は理解できない部品を書き直してしまうが、こちらは原本を
# 正として変えた所だけ書き戻す — 見出し・表・目次・書式が壊れない。
import pathlib

from officework import doc

# 開くファイルは、この .py の隣から探します(どこで走らせても同じ)
ここ = pathlib.Path(__file__).resolve().parent

d = doc.Doc.open(ここ / "報告書.docx")
print("段落:", len(d), "/ 表:", len(d.tables))
print("読めなかった部品:", d.unsupported)

n = d.replace("業務", "作業")      # run の書式を保ったまま置換
print("置き換え:", n, "箇所")
print("表の左上:", d.tables[0][0][0].text)

d.save(ここ / "報告書_差し替え.docx")
