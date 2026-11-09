import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
const map=JSON.parse(fs.readFileSync(new URL('../data/map.json',import.meta.url)));
test('121 distinct neighborhoods with valid SVG paths and Fortaleza coordinates',()=>{assert.equal(map.length,121);assert.equal(new Set(map.map(n=>n.id)).size,121);for(const n of map){assert.ok(n.name);assert.match(n.path,/^M[\d.,LMZ]+Z$/);assert.ok(n.longitude > -38.7 && n.longitude < -38.3);assert.ok(n.latitude > -3.95 && n.latitude < -3.65);assert.ok(n.anchor.every(v=>v>=30&&v<=931));}});
