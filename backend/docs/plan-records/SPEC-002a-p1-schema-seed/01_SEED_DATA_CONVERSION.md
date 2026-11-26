---
model: haiku
effort: low
reason: "Mechanical: extract a supplied script, run it once, compare SHA-256 values, commit four JSON files"
---

# Task 01: Seed data conversion (`seed/*.json`)

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply the exact steps below without assumption or improvisation. Never edit a Thai string by hand. If any check fails, stop and report; do not "fix" data.
  4. If a command or check in this task fails (build, lint, format, script or hash mismatch), that is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence a lint with `#[allow]`, never improvise a fix.

Battleground: [01_SEED_DATA_CONVERSION/debate.md](../../battlegrounds/SPEC-002a-p1-schema-seed/01_SEED_DATA_CONVERSION/debate.md). The script is run once from a temp directory and is **not** committed (D07: the backend never reads `../frontend-client/` in committed scripts; `docs/deployment.md`: no loose scripts).

## 1. Action Items
- [ ] Verify the four source files are the ones this plan was written against (SHA-256).
- [ ] Extract the script from this file into a temp directory and verify its SHA-256.
- [ ] Run it once; verify the four output SHA-256 values and the printed counts.
- [ ] Delete the temp script. Commit only `backend/seed/cards.json`, `seed/daily.json`, `seed/card-art.json`, `seed/reading-config.json`.

## 2. Harness
None. The script's own checks are the gate:
- exact key sets of every source record, so an unknown new field stops the run;
- a `reading.ts` round-trip: every parsed constant is re-rendered as its TypeScript source line and must occur verbatim in `reading.ts`;
- love-status keys, situation keys and positions are compared with the tables in `docs/specs/SPEC-002-master-data-to-backend.md` section 4.3, and the labels must match `reading.ts` and `daily-cards.json`;
- every string in the output must be a source string, an English key, or a suggestion ID;
- no output may contain a dropped token (`chatgpt.com`, `"source_url"`, `"raw_file"`, `"source"`, `"sourceFile"` and the rest of the list).

## 3. Tests
No Rust tests in this task. The seed hygiene tests (no `chatgpt.com`, `source_url`, `raw_file` or any other dropped field in `seed/`) are written in task 04 (`tests/seed_files.rs`) and run against these files.

## 4. Red (preconditions, run before converting)
Run from the monorepo root (`mmk-platform/`):
```bash
cd "$(git rev-parse --show-toplevel)"
test ! -e backend/seed/cards.json && echo "seed absent: OK"
shasum -a 256 frontend-client/src/lib/reading-cards.json frontend-client/src/lib/daily-cards.json \
  frontend-client/src/lib/reading.ts frontend-client/public/cards/v3/manifest.json
```
Expected: `seed absent: OK`, then exactly:
```
27dbe22cfea27bee938fc1af7098e5b0d86703120740ae05df8f342af9396f13  frontend-client/src/lib/reading-cards.json
f880baf1973cd4fd891b1bf0f3690abea2ed1d1a5eb3ed03f62b7a0ed136ce02  frontend-client/src/lib/daily-cards.json
e7fbd18e67411bba4143074f38ae298f07b9f15ef263246ef926cb56c2492e1f  frontend-client/src/lib/reading.ts
5610d0713b89378b1070845211f3ebe524be57e607f5b0140009b4789f427bc6  frontend-client/public/cards/v3/manifest.json
```
If any source hash differs, STOP and report to the owner: the frontend data changed after planning, and the output hashes in section 6 no longer apply. Do not continue with different sources.

## 5. Implementation

### Step 1: extract the script (do not retype it)
The command below copies the fenced `python` block of this task file into a temp directory, so no character is retyped:
```bash
cd "$(git rev-parse --show-toplevel)"
WORK="$(mktemp -d)"
awk '/^```python$/{f=1;next} /^```$/{if(f){exit}} f' \
  backend/docs/plans/SPEC-002a-p1-schema-seed/01_SEED_DATA_CONVERSION.md > "$WORK/convert_seed.py"
shasum -a 256 "$WORK/convert_seed.py"
```
Expected hash: `32ec4496cdbfaa84f24dae9bdc909a62c573d2770915691e59ecf163cc418eef`. If it differs, STOP.

### Step 2: run it once
```bash
python3 -I "$WORK/convert_seed.py" "$PWD/frontend-client" "$PWD/backend"
```
Expected output, exactly:
```
wrote seed/cards.json
wrote seed/daily.json
wrote seed/card-art.json
wrote seed/reading-config.json
cards=78 daily_cards=78 card_art=74 categories=4 situations=15 suggestions=12 positions=3 love_statuses=6 null_daily_love_texts=22
```
(`situations=15` is 9 work/money/life situations plus the 6 love situations derived from love statuses.)

### Step 3: delete the script
```bash
rm -rf "$WORK"
```

### The script (`convert_seed.py`, run once, never committed)
```python
"""One-off conversion of frontend-client master data into backend/seed/*.json (SPEC-002a, D07).

Usage: python3 -I convert_seed.py <frontend-client dir> <backend dir>
Writes <backend>/seed/{cards,daily,card-art,reading-config}.json. Every Thai string is copied
from the sources unchanged; only the SPEC-002 section 4.2 keep/drop table and the section 4.3
English keys are applied. Exits non-zero on any unexpected shape.
"""

import json
import re
import sys
from pathlib import Path

READING_KEYS = ["id", "name", "keyword", "meanings", "source_message", "source_line",
                "source_url", "review_status", "art", "details", "details_status"]
DAILY_TOP_KEYS = ["schema_version", "version", "updated", "status", "scope",
                  "auto_fallback_to_general_or_spread", "love_statuses", "cards"]
DAILY_CARD_KEYS = ["card_id", "name", "arcana", "reading_mode", "time_horizon", "orientation",
                   "content_status", "approved_for_web", "content", "missing_fields", "source",
                   "variants"]
DAILY_CONTENT_KEYS = ["theme", "overview", "work", "money", "love_by_status", "rabbit_message",
                      "colors", "affirmation"]
MANIFEST_TOP_KEYS = ["version", "capturedAt", "selection", "cards", "duplicates", "ignored"]
MANIFEST_CARD_KEYS = ["id", "sourceFile", "sourceSha256", "art", "sha256", "bytes", "width",
                      "height", "renderedWidth", "renderedHeight"]
# reading.ts order of `contexts` (SPEC-002 section 4.3: reading_sort_order).
READING_LOVE_KEYS = ["single", "crush", "talking", "complicated", "partnered", "ex_partner"]
CATEGORY_KEYS = ["love", "work", "money", "life"]
POSITION_KEYS = ["past", "present", "future"]
DROPPED_TOKENS = ["chatgpt.com", '"source_url"', '"source_message"', '"source_line"',
                  '"review_status"', '"details_status"', '"source"', '"raw_file"',
                  '"sourceFile"', '"sourceSha256"', '"capturedAt"', '"selection"',
                  '"duplicates"', '"ignored"']


def fail(msg):
    sys.exit(f"convert_seed: {msg}")


def expect_keys(obj, keys, where):
    if list(obj.keys()) != keys:
        fail(f"{where}: keys {list(obj.keys())} != expected {keys}")


def block(src, start, end_marker):
    i = src.index(start)
    j = src.index(end_marker, i)
    return src[i:j + len(end_marker)]


def quoted_list(text, where):
    items = re.findall(r"'([^'\\]*)'", text)
    if not items:
        fail(f"{where}: no string literals")
    return items


def parse_reading_ts(src):
    if "\\" in block(src, "export const contexts", "export type ReadingRequest"):
        fail("reading.ts: escape sequences in constants are not supported")
    contexts = quoted_list(block(src, "export const contexts = [", "] as const;"), "contexts")
    cat_block = block(src, "export const categories", "];")
    categories = re.findall(
        r"\{id: '([a-z]+)', label: '([^']*)', icon: '([^']*)', hint: '([^']*)'\}", cat_block)
    sit_block = block(src, "export const situations", "};")
    sug_block = block(src, "export const suggestions", "};")
    situations = {k: quoted_list(v, f"situations.{k}")
                  for k, v in re.findall(r"^  (work|money|life): \[(.*)\],$", sit_block, re.M)}
    suggestions = {k: quoted_list(v, f"suggestions.{k}")
                   for k, v in re.findall(r"^  (love|work|money|life): \[(.*)\],$", sug_block, re.M)}
    if "  love: contexts," not in sit_block:
        fail("reading.ts: situations.love is no longer `contexts`")
    positions = quoted_list(block(src, "export const positions = [", "] as const;"), "positions")
    return contexts, categories, situations, suggestions, positions


def check_reading_ts_roundtrip(src, contexts, categories, situations, suggestions, positions):
    """Rebuild each reading.ts source line from the parsed values; each must occur verbatim."""
    def lit(items):
        return ", ".join(f"'{s}'" for s in items)
    lines = [f"export const contexts = [{lit(contexts)}] as const;",
             f"export const positions = [{lit(positions)}] as const;"]
    lines += [f"  {{id: '{k}', label: '{l}', icon: '{i}', hint: '{h}'}},"
              for k, l, i, h in categories]
    lines += [f"  {k}: [{lit(v)}]," for k, v in situations.items()]
    lines += [f"  {k}: [{lit(v)}]," for k, v in suggestions.items()]
    for line in lines:
        if line not in src:
            fail(f"reading.ts round-trip failed for line: {line}")
    if [c[0] for c in categories] != CATEGORY_KEYS:
        fail(f"categories {[c[0] for c in categories]} != {CATEGORY_KEYS}")
    if list(situations) != ["work", "money", "life"] or any(len(v) != 3 for v in situations.values()):
        fail("situations: expected 3 each for work, money, life")
    if list(suggestions) != CATEGORY_KEYS or any(len(v) != 3 for v in suggestions.values()):
        fail("suggestions: expected 3 each for love, work, money, life")
    if len(contexts) != 6 or len(positions) != 3:
        fail("contexts must have 6 entries and positions 3")


def parse_spec_tables(spec):
    sec = block(spec, "### 4.3 English keys", "### 4.4")
    love = re.findall(r"^\| `([a-z_]+)` \| (.+?) \| (.+?) \|$", sec, re.M)
    sits = re.findall(r"^\| (work|money|life) \| `([a-z_]+)` \| (.+?) \|$", sec, re.M)
    pos = re.findall(r"`(past|present|future)` \(([^)]+)\)", sec)
    if len(love) != 6 or len(sits) != 9 or len(pos) != 3:
        fail(f"spec tables: found {len(love)} love rows, {len(sits)} situation rows, {len(pos)} positions")
    return love, sits, pos


def string_leaves(obj):
    if isinstance(obj, dict):
        for v in obj.values():
            yield from string_leaves(v)
    elif isinstance(obj, list):
        for v in obj:
            yield from string_leaves(v)
    elif isinstance(obj, str):
        yield obj


def main():
    if len(sys.argv) != 3:
        fail("usage: convert_seed.py <frontend-client dir> <backend dir>")
    fe, be = Path(sys.argv[1]), Path(sys.argv[2])
    reading = json.loads((fe / "src/lib/reading-cards.json").read_text(encoding="utf-8"))
    daily = json.loads((fe / "src/lib/daily-cards.json").read_text(encoding="utf-8"))
    manifest = json.loads((fe / "public/cards/v3/manifest.json").read_text(encoding="utf-8"))
    ts = (fe / "src/lib/reading.ts").read_text(encoding="utf-8")
    spec = (be / "docs/specs/SPEC-002-master-data-to-backend.md").read_text(encoding="utf-8")

    contexts, categories, situations, suggestions, positions = parse_reading_ts(ts)
    check_reading_ts_roundtrip(ts, contexts, categories, situations, suggestions, positions)
    ctx_to_key = dict(zip(contexts, READING_LOVE_KEYS))

    # Love statuses: keys from reading.ts order, daily labels from daily-cards.json.
    daily_labels = {s["key"]: s["source_label"] for s in daily["love_statuses"]}
    if sorted(daily_labels) != sorted(READING_LOVE_KEYS):
        fail(f"daily love_statuses keys {sorted(daily_labels)} differ from {sorted(READING_LOVE_KEYS)}")
    love_statuses = [{"key": ctx_to_key[c], "daily_label": daily_labels[ctx_to_key[c]],
                      "reading_label": c} for c in contexts]

    spec_love, spec_sits, spec_pos = parse_spec_tables(spec)
    if [(s["key"], s["daily_label"], s["reading_label"]) for s in love_statuses] != list(spec_love):
        fail(f"love statuses differ from SPEC-002 section 4.3: {love_statuses} vs {spec_love}")

    # Cards (reading-cards.json order).
    if not isinstance(reading, list) or len(reading) != 78:
        fail("reading-cards.json must be a list of 78 cards")
    cards = []
    for c in reading:
        expect_keys(c, READING_KEYS, f"reading-cards {c.get('id')}")
        for field in ("meanings", "details"):
            if list(c[field].keys()) != contexts:
                fail(f"reading-cards {c['id']}.{field} keys {list(c[field])} != contexts")
        cards.append({
            "id": c["id"], "name": c["name"], "keyword": c["keyword"], "art": c["art"],
            "meanings": {ctx_to_key[k]: v for k, v in c["meanings"].items()},
            "details": {ctx_to_key[k]: v for k, v in c["details"].items()},
        })

    # Daily document (daily-cards.json order); only the per-card `source` object is dropped.
    expect_keys(daily, DAILY_TOP_KEYS, "daily-cards top level")
    daily_cards = []
    for d in daily["cards"]:
        expect_keys(d, DAILY_CARD_KEYS, f"daily-cards {d.get('card_id')}")
        expect_keys(d["content"], DAILY_CONTENT_KEYS, f"daily-cards {d['card_id']}.content")
        expect_keys(d["variants"], ["summary_theme", "overview_theme"], f"daily-cards {d['card_id']}.variants")
        daily_cards.append({k: d[k] for k in DAILY_CARD_KEYS if k != "source"})
    daily_out = {k: daily[k] for k in DAILY_TOP_KEYS if k != "cards"}
    daily_out["cards"] = daily_cards

    # Card art (manifest order).
    expect_keys(manifest, MANIFEST_TOP_KEYS, "manifest top level")
    art_cards = []
    for m in manifest["cards"]:
        expect_keys(m, MANIFEST_CARD_KEYS, f"manifest {m.get('id')}")
        art_cards.append({k: m[k] for k in ["id", "art", "width", "height", "renderedWidth",
                                             "renderedHeight", "sha256", "bytes"]})
    card_art = {"cards": art_cards}

    # Reading config (reading.ts).
    for (cat, key, label), (cat_ts, labels_ts) in zip(
            spec_sits, [(c, l) for c in ["work", "money", "life"] for l in situations[c]]):
        if cat != cat_ts or label != labels_ts:
            fail(f"situation {key}: spec has ({cat}, {label}), reading.ts has ({cat_ts}, {labels_ts})")
    sit_out = {"love": [{"key": s["key"], "label": s["reading_label"]} for s in love_statuses]}
    for cat in ["work", "money", "life"]:
        sit_out[cat] = [{"key": key, "label": label} for c, key, label in spec_sits if c == cat]
    if [p for p, _ in spec_pos] != POSITION_KEYS or [l for _, l in spec_pos] != positions:
        fail(f"positions differ: spec {spec_pos}, reading.ts {positions}")
    reading_config = {
        "categories": [{"key": k, "label": l, "icon": i, "hint": h} for k, l, i, h in categories],
        "situations": sit_out,
        "suggestions": {cat: [{"id": f"{cat}-{n}", "text": t} for n, t in enumerate(texts, start=1)]
                        for cat, texts in suggestions.items()},
        "positions": [{"key": k, "label": l} for k, l in zip(POSITION_KEYS, positions)],
        "uncategorized_situations": [],
        "uncategorized_suggestions": [],
        "love_statuses": love_statuses,
    }

    outputs = {"cards.json": cards, "daily.json": daily_out, "card-art.json": card_art,
               "reading-config.json": reading_config}

    # Every output string must be a source string, an English key/id, or a reading.ts label.
    allowed = set(string_leaves(reading)) | set(string_leaves(daily)) | set(string_leaves(manifest))
    allowed |= set(contexts) | set(positions) | {x for c in categories for x in c}
    allowed |= {x for v in situations.values() for x in v} | {x for v in suggestions.values() for x in v}
    allowed |= {k for _, k, _ in spec_sits} | set(POSITION_KEYS) | set(READING_LOVE_KEYS)
    allowed |= {f"{c}-{n}" for c in CATEGORY_KEYS for n in (1, 2, 3)}
    for name, obj in outputs.items():
        for s in string_leaves(obj):
            if s not in allowed:
                fail(f"{name}: string not found in any source: {s!r}")

    seed_dir = be / "seed"
    seed_dir.mkdir(exist_ok=True)
    for name, obj in outputs.items():
        text = json.dumps(obj, ensure_ascii=False, indent=2) + "\n"
        for token in DROPPED_TOKENS:
            if token in text:
                fail(f"{name}: contains dropped token {token}")
        (seed_dir / name).write_text(text, encoding="utf-8")
        print(f"wrote seed/{name}")

    nulls = sum(1 for d in daily_cards for v in d["content"]["love_by_status"].values() if v is None)
    print(f"cards={len(cards)} daily_cards={len(daily_cards)} card_art={len(art_cards)} "
          f"categories={len(categories)} situations={sum(len(v) for v in sit_out.values())} "
          f"suggestions={sum(len(v) for v in suggestions.values())} positions={len(positions)} "
          f"love_statuses={len(love_statuses)} null_daily_love_texts={nulls}")


if __name__ == "__main__":
    main()
```

## 6. Green + Standards Check
```bash
cd "$(git rev-parse --show-toplevel)"
shasum -a 256 backend/seed/*.json
grep -c -E 'chatgpt\.com|source_url|raw_file|sourceFile|capturedAt' backend/seed/*.json
git status --short backend/seed
```
Expected:
```
1becdafad22a8038bdb1f460d260b83bb1d7dc6d033bb2840408169698fe23a3  backend/seed/card-art.json
6bb7b511e2baa147431d7de54936ad8e33002649cc3e5cbee3961ad5dfbf2a3c  backend/seed/cards.json
b2d8dd775dd6a0c0077eebc6745ec87805fad916a4aa9ebd21a1dca3f1dd22ba  backend/seed/daily.json
c7e6bfd157c8ea0e89448bba2d6f5236e6c735c1d43b47a3ffa6c81600b77725  backend/seed/reading-config.json
backend/seed/card-art.json:0
backend/seed/cards.json:0
backend/seed/daily.json:0
backend/seed/reading-config.json:0
?? backend/seed/
```
The `grep -c` line exits with status 1 because nothing matched; that is the expected result. No Rust code changes in this task, so no cargo check is needed. Commit only the four JSON files.
