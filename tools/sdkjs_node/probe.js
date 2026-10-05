// Line breaks of Japanese text in the sdkjs paragraph layout. Every
// character is 0.5 * 10 = 5 mm wide (the test measurer), so a width of
// n * 5 mm holds n characters
const g = require("./run.js");
const AscWord = g.AscWord, AscTest = g.AscTest;
const cw = AscTest.CharWidth * AscTest.FontSize;
const dc = new AscWord.CDocumentContent();
dc.ClearContent(false);
const para = new AscWord.Paragraph();
dc.AddToContent(0, para);
const run = new AscWord.CRun();
para.AddToContent(0, run);
function lines(text, n) {
  run.ClearContent();
  run.AddText(text);
  dc.Reset(0, 0, cw * n, 10000);
  dc.Recalculate_Page(0, true);
  const r = [];
  for (let i = 0; i < para.GetLinesCount(); ++i) r.push(para.GetTextOnLine(i));
  return r;
}
const cases = JSON.parse(require("fs").readFileSync(process.argv[2], "utf8"));
for (const [text, n] of cases) console.log(n, JSON.stringify(text), "=>", JSON.stringify(lines(text, n)));
