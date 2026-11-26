// The master data the UI renders, built either from the bundled JSON (fallback) or from the client API
// (backend SPEC-002a, docs/api/client/openapi.yaml v0.2.1). Pure and import-free so `node --test` can
// load it directly (src/lib/master-data-adapter.test.mjs).

export type CategoryId = 'love' | 'work' | 'money' | 'life';
const CATEGORY_IDS: readonly CategoryId[] = ['love', 'work', 'money', 'life'];
const DECK_SIZE = 78;
const POSITIONS = 3;

export type Card = {
  id: string;
  name: string;
  keyword: string;
  art: string | null;
  // Keyed by the 3-card love-status label shown in the UI (for example 'แอบชอบ').
  meanings: Record<string, string | null>;
  details: Record<string, string | null>;
  // Provenance of the bundled text. The API does not serve it (SPEC-002 section 4.2), so API cards have null.
  source: {url: string; message: number; line: number} | null;
};

export type DailyContent = {
  theme: string;
  overview: string;
  work: string;
  money: string;
  // Keyed by love-status key (for example 'crush'); null where no text exists.
  love_by_status: Record<string, string | null>;
  rabbit_message: string;
  colors: string;
  affirmation: string;
};
export type DailyCard = {card_id: string; name: string; content: DailyContent};

export type ArtEntry = {id: string; art: string; width: number; height: number; renderedWidth: number; renderedHeight: number};

export type ReadingConfig = {
  categories: {id: CategoryId; label: string; icon: string; hint: string}[];
  // Category to situation labels; `love` holds the 3-card love labels.
  situations: Record<CategoryId, string[]>;
  suggestions: Record<CategoryId, string[]>;
  positions: string[];
  // 3-card love labels in reading order.
  loveLabels: string[];
  // Daily-screen love statuses in daily order.
  dailyStatuses: {key: string; label: string}[];
};

export type MasterData = {source: 'bundled' | 'api'; cards: Card[]; daily: DailyCard[]; art: ArtEntry[]; config: ReadingConfig};

// ---- Bundled JSON (src/lib/*.json, public/cards/v3/manifest.json, src/lib/reading-config.ts) ----

type BundledInput = {
  readingCards: readonly {
    id: string; name: string; keyword: string; art: string | null;
    meanings: Record<string, string>; details: Record<string, string>;
    source_url: string; source_message: number; source_line: number;
  }[];
  dailyCards: {love_statuses: readonly {key: string; source_label: string}[]; cards: readonly DailyCard[]};
  artManifest: {cards: readonly ArtEntry[]};
  taxonomy: {
    categories: readonly {id: CategoryId; label: string; icon: string; hint: string}[];
    contexts: readonly string[];
    situations: Record<CategoryId, readonly string[]>;
    suggestions: Record<CategoryId, readonly string[]>;
    positions: readonly string[];
  };
};

export function fromBundled({readingCards, dailyCards, artManifest, taxonomy}: BundledInput): MasterData {
  return {
    source: 'bundled',
    cards: readingCards.map(c => ({
      id: c.id, name: c.name, keyword: c.keyword, art: c.art,
      meanings: {...c.meanings}, details: {...c.details},
      source: {url: c.source_url, message: c.source_message, line: c.source_line},
    })),
    daily: dailyCards.cards.map(c => ({card_id: c.card_id, name: c.name, content: {...c.content, love_by_status: {...c.content.love_by_status}}})),
    art: artManifest.cards.map(a => ({id: a.id, art: a.art, width: a.width, height: a.height, renderedWidth: a.renderedWidth, renderedHeight: a.renderedHeight})),
    config: {
      categories: taxonomy.categories.map(c => ({...c})),
      situations: mapCategories(id => [...taxonomy.situations[id]]),
      suggestions: mapCategories(id => [...taxonomy.suggestions[id]]),
      positions: [...taxonomy.positions],
      loveLabels: [...taxonomy.contexts],
      dailyStatuses: dailyCards.love_statuses.map(s => ({key: s.key, label: s.source_label})),
    },
  };
}

// ---- API bodies (GET /v1/cards, /v1/daily-cards, /v1/card-art, /v1/reading-config) ----

export type ApiBodies = {cards: unknown; daily: unknown; art: unknown; config: unknown};

// Throws when any body does not have the shape the UI needs; the caller then keeps the bundled data.
// All four bodies are adopted together or not at all, so the screens never mix two versions.
export function adaptApiMasterData(bodies: ApiBodies): MasterData {
  const statuses = list(field(bodies.config, 'love_statuses', 'reading-config'), 'reading-config.love_statuses').map((s, i) => {
    const where = `reading-config.love_statuses[${i}]`;
    return {key: text(field(s, 'key', where), `${where}.key`), dailyLabel: text(field(s, 'daily_label', where), `${where}.daily_label`), readingLabel: text(field(s, 'reading_label', where), `${where}.reading_label`)};
  });
  if (statuses.length === 0) fail('reading-config.love_statuses is empty');
  const statusKeys = statuses.map(s => s.key);
  const readingLabelOf = new Map(statuses.map(s => [s.key, s.readingLabel]));

  const config = adaptConfig(bodies.config, statuses.map(s => s.readingLabel));
  const cards = adaptCards(bodies.cards, statusKeys, key => readingLabelOf.get(key)!);
  const {daily, dailyStatuses} = adaptDaily(bodies.daily, cards, statusKeys);
  const art = adaptArt(bodies.art, cards);
  return {source: 'api', cards, daily, art, config: {...config, dailyStatuses}};
}

function adaptConfig(body: unknown, loveLabels: string[]): Omit<ReadingConfig, 'dailyStatuses'> {
  const categories = list(field(body, 'categories', 'reading-config'), 'reading-config.categories').map((c, i) => {
    const where = `reading-config.categories[${i}]`;
    const id = text(field(c, 'key', where), `${where}.key`);
    if (!(CATEGORY_IDS as readonly string[]).includes(id)) fail(`${where}.key "${id}" is not a category the UI knows`);
    return {id: id as CategoryId, label: text(field(c, 'label', where), `${where}.label`), icon: text(field(c, 'icon', where), `${where}.icon`), hint: text(field(c, 'hint', where), `${where}.hint`)};
  });
  const ids = categories.map(c => c.id);
  if (ids.length !== CATEGORY_IDS.length || new Set(ids).size !== ids.length) fail('reading-config.categories must hold love, work, money and life once each');

  const situationsBody = record(field(body, 'situations', 'reading-config'), 'reading-config.situations');
  const suggestionsBody = record(field(body, 'suggestions', 'reading-config'), 'reading-config.suggestions');
  const situations = mapCategories(id => list(situationsBody[id], `reading-config.situations.${id}`)
    .map((s, i) => text(field(s, 'label', `situations.${id}[${i}]`), `reading-config.situations.${id}[${i}].label`)));
  for (const id of CATEGORY_IDS) if (situations[id].length === 0) fail(`reading-config.situations.${id} is empty`);
  if (situations.love.join('\n') !== loveLabels.join('\n')) fail('reading-config.situations.love must equal the love statuses in reading order');
  const suggestions = mapCategories(id => suggestionsBody[id] === undefined ? [] : list(suggestionsBody[id], `reading-config.suggestions.${id}`)
    .map((s, i) => text(field(s, 'text', `suggestions.${id}[${i}]`), `reading-config.suggestions.${id}[${i}].text`)));
  const positions = list(field(body, 'positions', 'reading-config'), 'reading-config.positions')
    .map((p, i) => text(field(p, 'label', `positions[${i}]`), `reading-config.positions[${i}].label`));
  if (positions.length !== POSITIONS) fail(`reading-config.positions must hold ${POSITIONS} positions`);
  return {categories, situations, suggestions, positions, loveLabels};
}

function adaptCards(body: unknown, statusKeys: string[], labelOf: (key: string) => string): Card[] {
  const cards = list(body, 'cards').map((c, i) => {
    const where = `cards[${i}]`;
    const art = field(c, 'art', where);
    if (art !== null && typeof art !== 'string') fail(`${where}.art is not a string or null`);
    return {
      id: text(field(c, 'id', where), `${where}.id`),
      name: text(field(c, 'name', where), `${where}.name`),
      keyword: text(field(c, 'keyword', where), `${where}.keyword`),
      art,
      meanings: loveTexts(field(c, 'meanings', where), statusKeys, labelOf, `${where}.meanings`),
      details: loveTexts(field(c, 'details', where), statusKeys, labelOf, `${where}.details`),
      source: null,
    };
  });
  const ids = new Set(cards.map(c => c.id));
  if (cards.length !== DECK_SIZE || ids.size !== DECK_SIZE) fail(`cards must hold ${DECK_SIZE} unique cards`);
  return cards;
}

function adaptDaily(body: unknown, cards: Card[], statusKeys: string[]): {daily: DailyCard[]; dailyStatuses: ReadingConfig['dailyStatuses']} {
  const dailyStatuses = list(field(body, 'love_statuses', 'daily-cards'), 'daily-cards.love_statuses').map((s, i) => {
    const where = `daily-cards.love_statuses[${i}]`;
    return {key: text(field(s, 'key', where), `${where}.key`), label: text(field(s, 'source_label', where), `${where}.source_label`)};
  });
  if ([...dailyStatuses.map(s => s.key)].sort().join('\n') !== [...statusKeys].sort().join('\n')) fail('daily-cards.love_statuses must hold the same keys as reading-config.love_statuses');
  const daily = list(field(body, 'cards', 'daily-cards'), 'daily-cards.cards').map((c, i) => {
    const where = `daily-cards.cards[${i}]`;
    const content = field(c, 'content', where);
    const at = (name: string) => text(field(content, name, `${where}.content`), `${where}.content.${name}`);
    return {
      card_id: text(field(c, 'card_id', where), `${where}.card_id`),
      name: text(field(c, 'name', where), `${where}.name`),
      content: {
        theme: at('theme'), overview: at('overview'), work: at('work'), money: at('money'),
        love_by_status: loveTexts(field(content, 'love_by_status', `${where}.content`), statusKeys, key => key, `${where}.content.love_by_status`),
        rabbit_message: at('rabbit_message'), colors: at('colors'), affirmation: at('affirmation'),
      },
    };
  });
  const dailyIds = daily.map(d => d.card_id);
  if (new Set(dailyIds).size !== dailyIds.length || dailyIds.length !== cards.length || !cards.every(c => dailyIds.includes(c.id))) fail('daily-cards must hold exactly one entry per card');
  return {daily, dailyStatuses};
}

function adaptArt(body: unknown, cards: Card[]): ArtEntry[] {
  const artOf = new Map(cards.map(c => [c.id, c.art]));
  return list(field(body, 'cards', 'card-art'), 'card-art.cards').map((a, i) => {
    const where = `card-art.cards[${i}]`;
    const id = text(field(a, 'id', where), `${where}.id`);
    const art = text(field(a, 'art', where), `${where}.art`);
    if (artOf.get(id) !== art) fail(`${where} does not match the art of card "${id}"`);
    const size = (name: string) => {
      const v = field(a, name, where);
      return typeof v === 'number' && Number.isInteger(v) && v > 0 ? v : fail(`${where}.${name} is not a positive integer`);
    };
    return {id, art, width: size('width'), height: size('height'), renderedWidth: size('renderedWidth'), renderedHeight: size('renderedHeight')};
  });
}

// ---- helpers ----

function mapCategories<T>(make: (id: CategoryId) => T): Record<CategoryId, T> {
  return {love: make('love'), work: make('work'), money: make('money'), life: make('life')};
}

function loveTexts(value: unknown, keys: string[], labelOf: (key: string) => string, where: string): Record<string, string | null> {
  const map = record(value, where);
  const out: Record<string, string | null> = {};
  for (const key of keys) {
    const v = map[key];
    if (v !== null && typeof v !== 'string') fail(`${where}.${key} is not a string or null`);
    out[labelOf(key)] = v;
  }
  return out;
}

function fail(message: string): never {
  throw new Error(`master data from the API was rejected: ${message}`);
}
function record(value: unknown, where: string): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : fail(`${where} is not an object`);
}
function field(value: unknown, name: string, where: string): unknown {
  return record(value, where)[name];
}
function list(value: unknown, where: string): unknown[] {
  return Array.isArray(value) ? value : fail(`${where} is not an array`);
}
function text(value: unknown, where: string): string {
  return typeof value === 'string' ? value : fail(`${where} is not a string`);
}
