# 報告書.docx の表を CSV に吸い上げる — docx を読む側の見本(アプリ不要)。
#
#   pip install officework
#   python3 表の吸い上げ.py
#
# 文書の中の表は d.tables[番号][行][欄].text で読める。
# 「Word の報告書から数字を拾って集計に回す」の入口。
import csv
from officework import doc

d = doc.Doc.open("報告書.docx")
print("表の数:", len(d.tables))

t = d.tables[0]
with open("報告書_表.csv", "w", newline="", encoding="utf-8") as f:
    w = csv.writer(f)
    for row in t:
        w.writerow([cell.text for cell in row])

print(f"報告書_表.csv に {len(t)} 行を書きました")
for row in t:
    print(" | ".join(cell.text for cell in row))
