// Run: npm run test:data (node --test; Node strips the TypeScript types of the imported .ts files).
// backend/seed/*.json are byte-for-byte the bodies the client API serves (backend golden tests and
// `make smoke-local` check that), so they stand in for live API responses here.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {existsSync, readFileSync} from 'node:fs';
import {adaptApiMasterData, fromBundled} from './master-data-adapter.ts';
import * as taxonomy from './reading-config.ts';

const lib = new URL('./', import.meta.url);
const seed = new URL('../../../backend/seed/', import.meta.url);
const json = (url) => JSON.parse(readFileSync(url, 'utf8'));
const apiBodies = () => ({
  cards: json(new URL('cards.json', seed)),
  daily: json(new URL('daily.json', seed)),
  art: json(new URL('card-art.json', seed)),
  config: json(new URL('reading-config.json', seed)),
});
const bundled = () => fromBundled({
  readingCards: json(new URL('reading-cards.json', lib)),
  dailyCards: json(new URL('daily-cards.json', lib)),
  artManifest: json(new URL('../../public/cards/v3/manifest.json', lib)),
  taxonomy,
});
const withoutProvenance = (data) => ({...data, source: null, cards: data.cards.map(c => ({...c, source: null}))});
const skip = !existsSync(seed) && 'backend/seed is not next to frontend-client';

test('API bodies adapt to exactly the data the UI renders from the bundled JSON', {skip}, () => {
  const api = adaptApiMasterData(apiBodies());
  assert.equal(api.source, 'api');
  assert.ok(api.cards.every(c => c.source === null), 'the API serves no provenance');
  assert.deepEqual(withoutProvenance(api), withoutProvenance(bundled()));
});

test('bundled data has the shape the screens rely on', () => {
  const data = bundled();
  assert.equal(data.cards.length, 78);
  assert.equal(data.daily.length, 78);
  assert.equal(data.art.length, 74);
  assert.deepEqual(data.config.categories.map(c => c.id), ['love', 'work', 'money', 'life']);
  assert.equal(data.config.positions.length, 3);
  assert.deepEqual(data.config.dailyStatuses.map(s => s.key), ['single', 'crush', 'talking', 'partnered', 'ex_partner', 'complicated']);
  assert.ok(data.cards.every(c => data.config.loveLabels.every(label => typeof c.meanings[label] === 'string')));
});

for (const [name, breakIt, message] of [
  ['a missing card', b => { b.cards.pop(); }, /78 unique cards/],
  ['an unknown category', b => { b.config.categories[3].key = 'health'; }, /not a category the UI knows/],
  ['a daily entry for an unknown card', b => { b.daily.cards[0].card_id = 'x-card'; }, /one entry per card/],
  ['a meaning missing a love status', b => { delete b.cards[0].meanings.crush; }, /meanings\.crush/],
  ['art that does not match its card', b => { b.art.cards[0].art = '/cards/v3/other.webp'; }, /does not match the art/],
  ['an error envelope instead of a body', b => { b.cards = {request_id: 'r', code: 'SERVICE_UNAVAILABLE', message: 'x'}; }, /cards is not an array/],
]) {
  test(`rejects ${name}`, {skip}, () => {
    const bodies = apiBodies();
    breakIt(bodies);
    assert.throws(() => adaptApiMasterData(bodies), message);
  });
}
