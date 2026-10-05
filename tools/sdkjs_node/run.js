// Load the sdkjs word editor and its test helpers in node, without a
// browser, so its layout tests can run here (set SDKJS to another checkout)
const fs = require("fs"), vm = require("vm"), path = require("path");
const SDK = path.resolve(process.env.SDKJS || path.join(__dirname, "../../vendor/sdkjs"));
// A stub that accepts any property access or call
function stub(name) {
  const f = function () { return stub(name + "()"); };
  return new Proxy(f, {
    get(t, k) {
      if (k === Symbol.toPrimitive) return () => 0;
      if (k === "length") return 0;
      if (k in t) return t[k];
      if (typeof k === "symbol") return undefined;
      return (t[k] = stub(name + "." + String(k)));
    },
    set(t, k, v) { t[k] = v; return true; },
    construct() { return stub("new " + name); },
  });
}
const g = globalThis;
g.window = g;
g.self = g;
g.navigator = { userAgent: "node", platform: "Linux", language: "en", appVersion: "" };
g.document = stub("document");
g.location = { href: "http://localhost/", search: "", protocol: "http:", host: "localhost", hostname: "localhost" };
g.Image = function () {};
g.XMLHttpRequest = function () { return stub("xhr"); };
g.Worker = function () { return stub("worker"); };
g.localStorage = { getItem() { return null; }, setItem() {} };
g.requestAnimationFrame = () => 0;
g.XRegExp = function (src, flags) { return new RegExp(src, (flags || "").replace("x", "") + (/[\\]p\{/.test(src) ? "u" : "")); };
g.XRegExp.escape = s => s.replace(/[-[\]{}()*+?.,\\^$|#\s]/g, "\\$&");
g.$ = stub("$"); g.jQuery = g.$;
g.QUnit = { module() {}, test(n, f) { g.__tests.push(f); }, config: {}, assert: {} };
g.__tests = [];
// The scripts in the order the develop build loads them (build/Gruntfile.js,
// writeScripts): the polyfill, applyDocumentChanges.js, then the word
// configuration's files. The Local/ files belong to the offline app only
const cfg = JSON.parse(fs.readFileSync(path.join(SDK, "configs/word.json"), "utf8")).sdk;
const files = ["vendor/polyfill.js", "common/applyDocumentChanges.js"]
  .concat(cfg.min, cfg.common, cfg.desktop.min, cfg.desktop.common)
  .filter(f => !f.includes("/Local/"))
  .map(f => path.join(SDK, f));
const pre = [SDK + "/vendor/xregexp-all-min.js"];
const helpers = ["common.js", "editor.js", "document.js", "measurer.js"].map(f => SDK + "/tests/word/common/" + f);
let failed = 0;
for (const f of pre.concat(files, helpers)) {
  try { vm.runInThisContext(fs.readFileSync(f, "utf8"), { filename: f }); }
  catch (e) { failed++; if (failed < 15) console.error("LOAD", path.relative(SDK, f), String(e).slice(0, 160)); }
}
console.error("loaded", files.length + helpers.length, "failed", failed);
module.exports = g;
