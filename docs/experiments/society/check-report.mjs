import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';

const html = await fs.readFile('src/simulation/society/report.html', 'utf8');
const report = JSON.parse(await fs.readFile('target/society-evidence/baseline.json', 'utf8'));
const script = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const elements = new Map();
const downloads = [];
const blobs = new Map();
const element = id => ({
    id, textContent:'', innerHTML:'', value:'', disabled:false, handlers:{}, dataset:{},
    addEventListener(event, handler) { this.handlers[event] = handler; },
    setAttribute() {},
    insertAdjacentHTML(position, content) { this.innerHTML += content; },
    querySelector() { return {focus() {}}; },
    click() { downloads.push({name:this.download, blob:blobs.get(this.href)}); },
});
for (const match of html.matchAll(/id="([^"]+)"/g)) elements.set(match[1], element(match[1]));
elements.get('society-report').textContent = JSON.stringify(report);
elements.get('map-mode').value = 'activity';
elements.get('neighborhood').value = 'all';
elements.get('speed').value = '250';
const buttons = [...html.matchAll(/data-lesson="([^"]+)"/g)].map(match => ({...element(match[1]), dataset:{lesson:match[1]}}));
const context = vm.createContext({
    document:{getElementById:id => elements.get(id), querySelectorAll:() => buttons, createElement:tag => element(tag)},
    Blob,
    URL:{createObjectURL(blob) { const id = `blob:${blobs.size}`; blobs.set(id,blob); return id; }, revokeObjectURL() {}},
    setTimeout() {}, setInterval() { return 1; }, clearInterval() {},
});
vm.runInContext(script, context);
assert.equal(elements.get('population').textContent, '128');
assert.equal((elements.get('map').innerHTML.match(/data-identity=/g) ?? []).length, report.scenario.capacity);
for (let index=0; index<report.rounds.length; index++) {
    vm.runInContext(`state.index=${index};render();`,context);
    assert.equal(Number(elements.get('population').textContent.replaceAll(',','')), report.rounds[index].summary.population);
    assert.ok(!elements.get('chart').innerHTML.includes('NaN'));
    const cooperation = parseInt(elements.get('cooperation').textContent);
    assert.ok(Number.isNaN(cooperation) || cooperation <= 100);
}
const deathDay = report.rounds.findIndex((round,index) => index < report.rounds.length - 1 && round.agents.some(agent => agent.life === 'Dead'));
const dead = report.rounds[deathDay].agents.find(agent => agent.life === 'Dead');
vm.runInContext(`state.index=${deathDay + 1};state.identity=${dead.identity};render();`,context);
assert.ok(elements.get('detail').innerHTML.includes('No longer present'));
const birthDay = report.rounds.findIndex(round => round.agents.some(agent => agent.parent_identity > 0));
const newborn = report.rounds[birthDay].agents.find(agent => agent.parent_identity > 0);
vm.runInContext(`state.index=${birthDay - 1};state.identity=${newborn.identity};render();`,context);
assert.ok(elements.get('detail').innerHTML.includes('has not appeared yet'));
for (const button of buttons) button.handlers.click();
assert.equal(downloads.length, 8);
for (const download of downloads) {
    const scenario = JSON.parse(await download.blob.text());
    assert.deepEqual(scenario.agents, report.scenario.agents);
    assert.equal(scenario.initial_food, report.scenario.initial_food);
    const wageCase = /lesson-(posted|negotiated)-/.test(download.name);
    assert.ok(!wageCase || (scenario.wage_offer === 2 && scenario.reservation_wage === 4 && scenario.maximum_wage === 8));
}
console.log('Script/data checks passed: 120 recorded days, map capacity, no invalid chart coordinates, cooperation denominator, dead/future identity selection, 8 lesson scenario downloads.');
console.log('These use a DOM stub and actual GPU output; browser rendering and native downloads remain unverified.');
