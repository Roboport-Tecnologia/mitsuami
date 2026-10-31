//! The review page: every pending change with its baseline and capture,
//! four ways to compare them, why the pixels changed, and what the review
//! decided for it so far.

use crate::changes::{Change, Decision};

/// Where the page gets images from, and whether it can act.
pub enum Mode<'a> {
    /// Served by `visual review`: images from the server, and Accept and
    /// Reject call it back.
    Live { token: &'a str },
    /// Written to a folder, images beside it. Read-only.
    Static,
}

/// The image `kind` (`baseline`, `capture` or `diff`) of change `index`.
pub fn image_url(mode: &Mode, index: usize, kind: &str) -> String {
    match mode {
        Mode::Live { token } => format!("/image/{index}/{kind}?token={token}"),
        Mode::Static => format!("images/{index}-{kind}.png"),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `decisions` has one for each change, `None` while it's pending.
pub fn page(changes: &[Change], decisions: &[Option<Decision>], mode: &Mode) -> String {
    let live = matches!(mode, Mode::Live { .. });
    let mut cards = String::new();
    for (index, change) in changes.iter().enumerate() {
        let image = |kind: &str| image_url(mode, index, kind);
        let decided = match decisions.get(index).copied().flatten() {
            Some(Decision::Accepted) => " accepted",
            Some(Decision::Rejected) => " rejected",
            None => "",
        };
        let status = if change.is_new() { "new" } else { "changed" };
        let actions = if live {
            format!(
                r#"<div class="actions">
          <button class="accept" data-action="accept" data-index="{index}">Accept</button>
          <button class="reject" data-action="reject" data-index="{index}">Reject</button>
          <button class="undo" data-action="undo" data-index="{index}">Undo</button>
        </div>"#
            )
        } else {
            String::new()
        };
        let views = if change.is_new() {
            format!(r#"<div class="stage"><figure><img src="{}" alt="New capture"></figure></div>"#, image("capture"))
        } else {
            let diff = if change.diff().exists() {
                format!(r#"<img class="view-diff" src="{}" alt="Diff">"#, image("diff"))
            } else {
                r#"<p class="view-diff note">The capture's size changed, so there is no diff image.</p>"#.to_owned()
            };
            format!(
                r#"<div class="tabs" role="tablist">
          <button role="tab" data-view="side" aria-selected="true">Side by side</button>
          <button role="tab" data-view="swipe" aria-selected="false">Swipe</button>
          <button role="tab" data-view="onion" aria-selected="false">Onion skin</button>
          <button role="tab" data-view="diff" aria-selected="false">Diff</button>
        </div>
        <div class="stage" data-view="side">
          <div class="view-side">
            <figure><figcaption>Baseline</figcaption><img src="{baseline}" alt="Baseline"></figure>
            <figure><figcaption>New</figcaption><img src="{capture}" alt="New capture"></figure>
          </div>
          <div class="view-stack view-swipe">
            <div class="frame"><img src="{baseline}" alt="Baseline"><img class="top" src="{capture}" alt="New capture"></div>
            <input type="range" min="0" max="100" value="50" aria-label="Swipe between baseline and new">
          </div>
          <div class="view-stack view-onion">
            <div class="frame"><img src="{baseline}" alt="Baseline"><img class="top" src="{capture}" alt="New capture"></div>
            <input type="range" min="0" max="100" value="50" aria-label="Opacity of the new capture">
          </div>
          {diff}
        </div>"#,
                baseline = image("baseline"),
                capture = image("capture"),
            )
        };
        cards.push_str(&format!(
            r#"
    <article class="change{decided}" id="change-{index}" data-status="{status}" data-scale="{scale}">
      <header>
        <div>
          <h2>{name}</h2>
          <p class="where"><span class="tag">{backend}</span><span class="tag">{image_name}</span><span class="tag {status}">{status}</span></p>
        </div>
        {actions}
      </header>
      {notes}<pre class="why">{why}</pre>
      {views}
      <p class="path">{path}</p>
    </article>"#,
            name = escape(&change.name),
            scale = change.scale(),
            backend = escape(&change.backend),
            image_name = escape(&change.image),
            notes = change.notes().map_or(String::new(), |notes| {
                let paragraphs: String =
                    notes.split("\n\n").map(|p| format!("<p>{}</p>", escape(p.trim()))).collect();
                format!(r#"<div class="notes"><h3>Expected</h3>{paragraphs}</div>"#)
            }),
            why = escape(change.explanation().trim_end()),
            path = escape(&change.display),
        ));
    }
    let summary = match changes.len() {
        0 => "Nothing to review: every capture matches its baseline.".to_owned(),
        1 => "1 change to review.".to_owned(),
        n => format!("{n} changes to review."),
    };
    let summary = if live && !changes.is_empty() {
        format!(
            "{summary} Nothing changes until Done, which accepts and rejects them as decided; until then a decision can be undone."
        )
    } else {
        summary
    };
    let bulk = if live && !changes.is_empty() {
        r#"<button class="accept" data-action="accept-all">Accept the rest</button><button data-action="done">Done</button>"#
    } else if !live && !changes.is_empty() {
        r#"<p class="note">Accept these with <code>cargo mitsuami visual accept</code>, or review them with <code>cargo mitsuami visual review</code>.</p>"#
    } else {
        ""
    };
    let token = match mode {
        Mode::Live { token } => *token,
        Mode::Static => "",
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Visual review</title>
<link rel="icon" href="data:,">
<style>{STYLE}</style>
</head>
<body data-token="{token}">
  <main>
    <header class="top">
      <div>
        <h1>Visual review</h1>
        <p class="summary">{summary}</p>
      </div>
      <div class="bulk">{bulk}</div>
    </header>{cards}
  </main>
  <script>{SCRIPT}</script>
</body>
</html>
"#
    )
}

/// The accepted baselines, a section per snapshot with a tile per machine
/// image side by side, to reject any: `decisions` as in [`page`].
pub fn baselines_page(baselines: &[Change], decisions: &[Option<Decision>], token: &str) -> String {
    let mode = Mode::Live { token };
    let mut names: Vec<&str> = baselines.iter().map(|b| b.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let mut images: Vec<String> = baselines.iter().map(|b| format!("{}/{}", b.backend, b.image)).collect();
    images.sort_unstable();
    images.dedup();
    let mut sections = String::new();
    for name in &names {
        let mut tiles = String::new();
        for (index, baseline) in baselines.iter().enumerate().filter(|(_, b)| b.name == *name) {
            let rejected =
                if decisions.get(index).copied().flatten() == Some(Decision::Rejected) { " rejected" } else { "" };
            let machine = format!("{}/{}", baseline.backend, baseline.image);
            tiles.push_str(&format!(
                r#"
        <figure class="tile{rejected}" id="change-{index}" data-image="{machine}" data-scale="{scale}">
          <figcaption><span>{machine}</span>
            <button class="reject" data-action="reject" data-index="{index}">Reject</button>
            <button class="undo" data-action="undo" data-index="{index}">Undo</button>
          </figcaption>
          <img loading="lazy" src="{src}" alt="{name} on {machine}">
        </figure>"#,
                machine = escape(&machine),
                scale = baseline.scale(),
                src = image_url(&mode, index, "baseline"),
                name = escape(name),
            ));
        }
        sections.push_str(&format!(
            r#"
    <section class="snapshot" data-name="{name}">
      <h2>{name}</h2>
      <div class="tiles">{tiles}
      </div>
    </section>"#,
            name = escape(name),
        ));
    }
    let filters: String = images
        .iter()
        .map(|image| {
            let image = escape(image);
            format!(r#"<label><input type="checkbox" data-image="{image}" checked> {image}</label>"#)
        })
        .collect();
    let summary = format!(
        "{} baselines of {} snapshots. Nothing changes until Done, which deletes the rejected ones; the next run captures them again, for review.",
        baselines.len(),
        names.len()
    );
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Visual baselines</title>
<link rel="icon" href="data:,">
<style>{STYLE}{BASELINES_STYLE}</style>
</head>
<body data-token="{token}">
  <main>
    <header class="top">
      <div>
        <h1>Visual baselines</h1>
        <p class="summary">{summary}</p>
      </div>
      <div class="bulk"><button data-action="done">Done</button></div>
    </header>
    <div class="filters">
      <input type="search" id="filter" placeholder="Filter by name" aria-label="Filter by name">
      {filters}
    </div>{sections}
  </main>
  <script>{BASELINES_SCRIPT}</script>
</body>
</html>
"#
    )
}

const BASELINES_STYLE: &str = r#"
.filters { display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: center; margin-bottom: 16px; }
.filters input[type="search"] { font: inherit; padding: 6px 10px; border: 1px solid var(--line); border-radius: 6px;
  background: var(--card); color: var(--text); min-width: 240px; }
.snapshot { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 16px; margin-bottom: 16px; }
.tiles { display: flex; flex-wrap: wrap; gap: 16px; margin-top: 12px; align-items: flex-start; }
.tile { margin: 0; min-width: 0; }
.tile figcaption { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; color: var(--muted); font-size: 12px; }
.tile img { display: block; max-width: 100%; height: auto;
  background: repeating-conic-gradient(var(--check-a) 0 25%, var(--check-b) 0 50%) 0 0 / 16px 16px; }
.tile .undo { display: none; }
.tile.rejected img { opacity: .35; }
.tile.rejected figcaption::after { content: "Rejected"; color: var(--bad); }
.tile.rejected .reject { display: none; }
.tile.rejected .undo { display: inline-block; }
[hidden] { display: none !important; }
"#;

const BASELINES_SCRIPT: &str = r#"
const token = document.body.dataset.token;
// Captures are in physical pixels: show them at their logical size.
for (const tile of document.querySelectorAll(".tile")) {
  const img = tile.querySelector("img"), scale = Number(tile.dataset.scale) || 1;
  const size = () => { if (img.naturalWidth) img.style.width = `${img.naturalWidth / scale}px`; };
  img.complete ? size() : img.addEventListener("load", size);
}
const filter = document.getElementById("filter");
const filters = [...document.querySelectorAll(".filters input[type=checkbox]")];
function apply() {
  const text = filter.value.trim().toLowerCase();
  const shown = new Set(filters.filter(f => f.checked).map(f => f.dataset.image));
  for (const section of document.querySelectorAll(".snapshot")) {
    let any = false;
    for (const tile of section.querySelectorAll(".tile")) {
      tile.hidden = !shown.has(tile.dataset.image);
      any ||= !tile.hidden;
    }
    section.hidden = !any || !section.dataset.name.toLowerCase().includes(text);
  }
}
filter.addEventListener("input", apply);
for (const f of filters) f.addEventListener("change", apply);
// The server only records a decision; Done applies them all.
document.addEventListener("click", async (event) => {
  const button = event.target.closest("button[data-action]");
  if (!button) return;
  const { action, index } = button.dataset;
  if (action === "reject" || action === "undo") {
    const response = await fetch(`/${action}/${index}?token=${token}`, { method: "POST" });
    if (!response.ok) return alert(`Could not ${action}: ${await response.text()}`);
    document.getElementById(`change-${index}`).classList.toggle("rejected", action === "reject");
  }
  if (action === "done") {
    const response = await fetch(`/done?token=${token}`, { method: "POST" });
    if (!response.ok) return alert(`Some baselines weren't removed:\n${await response.text()}`);
    document.querySelector(".summary").textContent = "Done: the rejected baselines are removed. You can close this page.";
    for (const b of document.querySelectorAll("button")) b.disabled = true;
  }
});
"#;

const STYLE: &str = r#"
:root {
  --bg: #f6f6f4; --card: #ffffff; --text: #1d1d1b; --muted: #6b6b66; --line: #e2e2dd;
  --accent: #2f6fdb; --good: #1f8a4c; --bad: #c2362f; --warn: #b7791f;
  --check-a: #ffffff; --check-b: #e9e9e6;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #161615; --card: #1f1f1e; --text: #ededea; --muted: #9a9a94; --line: #33332f;
    --accent: #6f9ff0; --good: #4cc07e; --bad: #ec6b63; --warn: #e3a64a;
    --check-a: #2a2a28; --check-b: #232321;
    color-scheme: dark;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--text);
  font: 14px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; }
main { max-width: 1200px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { font-size: 22px; margin: 0; }
h2 { font-size: 15px; margin: 0; word-break: break-all; }
.top { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; margin-bottom: 20px; }
.summary, .note, .path { color: var(--muted); margin: 4px 0 0; }
.path { font: 12px ui-monospace, SFMono-Regular, Menlo, monospace; word-break: break-all; }
code { font: 12px ui-monospace, SFMono-Regular, Menlo, monospace; }
.bulk, .actions { display: flex; gap: 8px; align-items: center; }
button { font: inherit; border: 1px solid var(--line); background: var(--card); color: var(--text);
  border-radius: 6px; padding: 6px 12px; cursor: pointer; }
button:hover { border-color: var(--muted); }
button.accept { background: var(--good); border-color: var(--good); color: #fff; }
button.reject { color: var(--bad); }
button:disabled { opacity: .5; cursor: default; }
.change { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 16px; margin-bottom: 16px; }
.change > header { display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; flex-wrap: wrap; }
.where { margin: 6px 0 0; display: flex; gap: 6px; flex-wrap: wrap; }
.tag { font-size: 12px; padding: 1px 8px; border-radius: 999px; border: 1px solid var(--line); color: var(--muted); }
.tag.new { color: var(--accent); border-color: var(--accent); }
.tag.changed { color: var(--warn); border-color: var(--warn); }
.notes { border-left: 3px solid var(--accent); padding: 4px 12px; margin: 12px 0; }
.notes h3 { font-size: 12px; text-transform: uppercase; letter-spacing: .06em; color: var(--muted); margin: 0 0 4px; }
.notes p { margin: 0 0 6px; max-width: 75ch; }
.why { background: var(--bg); border-radius: 6px; padding: 8px 10px; margin: 12px 0; overflow-x: auto;
  font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; white-space: pre; }
.tabs { display: flex; gap: 4px; margin-bottom: 8px; flex-wrap: wrap; }
.tabs button { border-radius: 6px; padding: 4px 10px; }
.tabs button[aria-selected="true"] { background: var(--accent); border-color: var(--accent); color: #fff; }
.stage { overflow-x: auto; }
.stage img { display: block; max-width: 100%; height: auto; image-rendering: pixelated;
  background: repeating-conic-gradient(var(--check-a) 0 25%, var(--check-b) 0 50%) 0 0 / 16px 16px; }
figure { margin: 0; }
figcaption { color: var(--muted); font-size: 12px; margin-bottom: 4px; }
.view-side { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; }
.frame { position: relative; display: inline-block; max-width: 100%; }
.frame .top { position: absolute; top: 0; left: 0; }
.view-stack input { display: block; width: min(100%, 480px); margin-top: 8px; }
.stage > .view-side, .stage > .view-stack, .stage > .view-diff { display: none; }
.stage[data-view="side"] .view-side, .stage[data-view="swipe"] .view-swipe,
.stage[data-view="onion"] .view-onion { display: block; }
.stage[data-view="side"] .view-side { display: grid; }
.stage[data-view="diff"] .view-diff { display: block; }
.change.accepted .stage, .change.rejected .stage { opacity: .55; }
.actions .undo { display: none; }
.change.accepted .actions::before { content: "Accepted"; color: var(--good); }
.change.rejected .actions::before { content: "Rejected"; color: var(--bad); }
.change.accepted .actions button, .change.rejected .actions button { display: none; }
.change.accepted .actions .undo, .change.rejected .actions .undo { display: inline-block; }
"#;

const SCRIPT: &str = r#"
const token = document.body.dataset.token;
for (const change of document.querySelectorAll(".change")) {
  // Captures are in physical pixels: show them at their logical size.
  const scale = Number(change.dataset.scale) || 1;
  for (const img of change.querySelectorAll(".stage img")) {
    const size = () => { if (img.naturalWidth) img.style.width = `${img.naturalWidth / scale}px`; };
    img.complete ? size() : img.addEventListener("load", size);
  }
  const stage = change.querySelector(".stage");
  for (const tab of change.querySelectorAll("[data-view]")) {
    if (tab.tagName !== "BUTTON") continue;
    tab.addEventListener("click", () => {
      stage.dataset.view = tab.dataset.view;
      for (const t of change.querySelectorAll(".tabs button")) t.setAttribute("aria-selected", t === tab);
    });
  }
  const swipe = change.querySelector(".view-swipe");
  if (swipe) {
    const top = swipe.querySelector(".top"), range = swipe.querySelector("input");
    const apply = () => top.style.clipPath = `inset(0 0 0 ${range.value}%)`;
    range.addEventListener("input", apply); apply();
  }
  const onion = change.querySelector(".view-onion");
  if (onion) {
    const top = onion.querySelector(".top"), range = onion.querySelector("input");
    const apply = () => top.style.opacity = range.value / 100;
    range.addEventListener("input", apply); apply();
  }
}
// The server only records a decision; Done applies them all.
async function act(action, index) {
  const change = document.getElementById(`change-${index}`);
  const response = await fetch(`/${action}/${index}?token=${token}`, { method: "POST" });
  if (!response.ok) return alert(`Could not ${action}: ${await response.text()}`);
  change.classList.remove("accepted", "rejected");
  if (action !== "undo") change.classList.add(action === "accept" ? "accepted" : "rejected");
}
const decided = (c) => c.classList.contains("accepted") || c.classList.contains("rejected");
document.addEventListener("click", async (event) => {
  const button = event.target.closest("button[data-action]");
  if (!button) return;
  const { action, index } = button.dataset;
  if (action === "accept" || action === "reject" || action === "undo") await act(action, index);
  if (action === "accept-all") for (const c of document.querySelectorAll(".change")) if (!decided(c)) await act("accept", c.id.slice(7));
  if (action === "done") {
    const response = await fetch(`/done?token=${token}`, { method: "POST" });
    if (!response.ok) return alert(`Some decisions weren't applied:\n${await response.text()}`);
    document.querySelector(".summary").textContent = "Done: the decisions are applied. You can close this page.";
    for (const b of document.querySelectorAll("button")) b.disabled = true;
  }
});
"#;
