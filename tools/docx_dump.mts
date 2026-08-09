// genoffice の docx エンジン(packages/docx-engine)に docx を1枚通し、
// 比べられる物だけを JSON で吐く。tools/pydoc_diff.py から呼ばれる。
//
//   tsx tools/docx_dump.ts <文書.docx> <docx-engine/src/index.ts の絶対径路>
//
// **向こうの木には何も置かない。** 入口だけを動的 import で借りる
// (設計の「genoffice には何もしない」に従う)。
import { readFileSync } from 'node:fs'

const [docxPath, enginePath] = process.argv.slice(2)
const { parseDocx } = await import(enginePath)
const doc = await parseDocx(new Uint8Array(readFileSync(docxPath)))

const runText = (x: any): string => (x?.runs ?? []).map((r: any) => r.text ?? '').join('')
// セルは richParas(書式付き)を正とし、無ければ paras(平文)に落ちる
const cellText = (c: any): string =>
  c?.richParas ? c.richParas.map(runText).join('\n') : (c?.paras ?? []).join('\n')

const paragraphs: string[] = []
const tables: string[][][] = []
for (const b of doc.blocks) {
  if (b.type === 'table' && b.table) {
    // rows は「セルの配列」の配列(row.cells ではない)
    tables.push(b.table.rows.map((row: any) => row.map(cellText)))
  } else if (b.type !== 'passthrough') {
    paragraphs.push(runText(b))
  }
}

console.log(JSON.stringify({
  paragraphs,
  tables,
  header: (doc.headerParas ?? []).map(runText),
  footer: (doc.footerParas ?? []).map(runText),
}))
