// genoffice の保存の道に xlsx を1枚通す。**編集をひとつも渡さない。**
//
//   tsx tools/xlsx_save_probe.mts <元.xlsx> <出力.xlsx> <apps/sheets の絶対径路> <サイドカー>
//
// 開いて保存しただけの物を作るのが目的。段3(書き)を載せ替える価値があるかは
// 「向こうの保存が何かを壊すか」で決まる、と設計に書いた(2026-08-10)。
// **壊す証拠が出なければ、書きは向こうのままでよいと確定できる。**
//
// **向こうの木には何も置かない。** 入口を動的 import で借りるだけ
// (docx_dump.mts と同じ作法。設計の「genoffice には何もしない」)。
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { randomUUID } from 'node:crypto'

const [source, target, sheetsDir, sidecarPath, editSpec] = process.argv.slice(2)

// 5つめの引数があれば編集を1つ入れる。`シート名:行:列:打つ字`(行と列は0起点)。
// **計画層が実際に働いたときに周りを壊さないか**を見るのが目的で、
// 編集ゼロでは planCellEditsToXlsx のほとんどが素通りしてしまう
const edits = editSpec
  ? [
      (() => {
        const i = editSpec.lastIndexOf(':')
        const [sheetName, row, column] = editSpec.slice(0, i).split(':')
        const raw = editSpec.slice(i + 1)
        const n = Number(raw)
        return {
          sheetName,
          row: Number(row),
          column: Number(column),
          writeValue: true,
          // 数として読めるなら数で置く(向こうの CellScalar は number|string|boolean|null)
          cell: { value: Number.isFinite(n) && raw.trim() !== '' ? n : raw },
        }
      })(),
    ]
  : []

// 向こうの client と同じ喋り方をする最小の相手。**12 コマンドのうち、
// 保存が使う書庫まわりだけ**(archive_manifest / read_entries / save_archive)。
class Sidecar {
  private child = spawn(sidecarPath, [], { stdio: ['pipe', 'pipe', 'inherit'] })
  private lines = createInterface({ input: this.child.stdout })
  private waiting: ((v: any) => void)[] = []

  constructor() {
    this.lines.on('line', (l) => this.waiting.shift()?.(JSON.parse(l)))
  }

  private call(command: string, body: Record<string, unknown>): Promise<any> {
    return new Promise((resolve, reject) => {
      this.waiting.push((r) =>
        r.ok ? resolve(r.result) : reject(new Error(JSON.stringify(r.error))),
      )
      this.child.stdin.write(
        `${JSON.stringify({ version: 1, requestId: randomUUID(), command, ...body })}\n`,
      )
    })
  }

  archiveManifest = (path: string) => this.call('archive_manifest', { path })
  readEntries = (i: any) => this.call('read_entries', i)
  scanEntries = (i: any) => this.call('scan_entries', i)
  saveArchive = (i: any) => this.call('save_archive', i)
  convertWorkbook = (i: any) => this.call('convert_workbook', i)
  stop = () => this.child.kill()
}

const { saveWorkbookViaSidecar } = await import(`${sheetsDir}/src/gateway/xlsx-package-io.ts`)
const client = new Sidecar()
try {
  const r = await saveWorkbookViaSidecar({
    client,
    sourcePath: source,
    targetPath: target,
    edits: edits as any,
  })
  console.log(JSON.stringify({ ok: true, result: r ?? null }))
} catch (e) {
  console.log(JSON.stringify({ ok: false, error: String(e) }))
} finally {
  client.stop()
}
