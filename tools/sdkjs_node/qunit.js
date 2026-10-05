// Run sdkjs QUnit test files in node: node qunit.js file.js...
const fs = require("fs"), vm = require("vm");
const results = [];
let mod = "", hooks = {};
const tests = [];
const run = require("./run.js");
const g = globalThis;
g.$ = function (f) { if (typeof f === "function") f(); return g.$; };
g.QUnit = {
  module(name, h) { mod = name; hooks = (typeof h === "object" && h) || {}; if (typeof h === "function") h(hooks); },
  test(name, fn) { tests.push({ mod, name, fn, hooks }); },
  config: {}, assert: {}
};
for (const f of process.argv.slice(2)) vm.runInThisContext(fs.readFileSync(f, "utf8"), { filename: f });
let pass = 0, fail = 0;
for (const t of tests) {
  const fails = [];
  const assert = {
    strictEqual(a, b, m) { if (a !== b) fails.push(`${m || ""}: ${JSON.stringify(a)} !== ${JSON.stringify(b)}`); },
    equal(a, b, m) { if (a != b) fails.push(`${m || ""}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`); },
    deepEqual(a, b, m) { if (JSON.stringify(a) !== JSON.stringify(b)) fails.push(`${m || ""}: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`); },
    ok(a, m) { if (!a) fails.push(`${m || ""}: not ok`); },
    true(a, m) { if (a !== true) fails.push(`${m || ""}: not true`); },
    close(a, b, d, m) { if (Math.abs(a - b) > d) fails.push(`${m || ""}: ${a} not close to ${b}`); },
    expect() {}
  };
  try { if (t.hooks.beforeEach) t.hooks.beforeEach(); t.fn(assert); if (t.hooks.afterEach) t.hooks.afterEach(); }
  catch (e) { fails.push("threw " + String(e).slice(0, 200)); }
  if (fails.length) { fail++; console.log("FAIL", t.mod, "/", t.name); fails.slice(0, 5).forEach(x => console.log("   ", x)); }
  else pass++;
}
console.log("pass", pass, "fail", fail);
