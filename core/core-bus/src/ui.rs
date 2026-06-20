//! Minimal AOI dashboard — polls /stats and /results/recent.

use axum::response::Html;

pub async fn aoi_dashboard() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>SFI AOI Preview</title>
  <style>
    body { font-family: system-ui, sans-serif; margin: 1.5rem; background: #0f1419; color: #e7ecf1; }
    h1 { font-size: 1.25rem; margin-bottom: 0.5rem; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 0.75rem; }
    .card { background: #1a2332; border-radius: 8px; padding: 0.75rem 1rem; }
    .label { font-size: 0.75rem; opacity: 0.7; }
    .value { font-size: 1.4rem; font-weight: 600; }
    table { width: 100%; border-collapse: collapse; margin-top: 1rem; font-size: 0.9rem; }
    th, td { text-align: left; padding: 0.4rem 0.5rem; border-bottom: 1px solid #2a3544; }
    .ok { color: #3dd68c; }
    .ng { color: #ff6b6b; }
    input[type=number] { width: 5rem; padding: 0.25rem; }
    button { padding: 0.35rem 0.75rem; cursor: pointer; }
  </style>
</head>
<body>
  <h1>StructForIndustry — AOI Line Preview</h1>
  <div class="grid" id="stats"></div>
  <p>
    Threshold:
    <input type="number" id="threshold" min="0" max="255" value="128"/>
    <button onclick="applyThreshold()">Apply (hot reload)</button>
  </p>
  <table>
    <thead><tr><th>Frame</th><th>Verdict</th><th>Defects</th><th>Recipe</th><th>Time (ns)</th></tr></thead>
    <tbody id="rows"></tbody>
  </table>
  <script>
    async function refresh() {
      const [stats, results, profile, spc] = await Promise.all([
        fetch('/stats').then(r => r.json()),
        fetch('/results/recent').then(r => r.json()),
        fetch('/profile').then(r => r.json()).catch(() => null),
        fetch('/spc/metrics').then(r => r.json()).catch(() => null),
      ]);
      const s = document.getElementById('stats');
      const items = [
        ['Frames', stats.frames_received],
        ['Task done', stats.task_done_published],
        ['SPC pub', stats.spc_metrics_published],
        ['MES sent', stats.mes_reports_sent],
        ['NG rate', spc?.values?.find(v => v.name === 'ng_rate')?.value?.toFixed(2) ?? '-'],
      ];
      if (profile) {
        document.getElementById('threshold').value = profile.vision?.threshold ?? profile.threshold ?? 128;
      }
      s.innerHTML = items.map(([l,v]) =>
        `<div class="card"><div class="label">${l}</div><div class="value">${v ?? 0}</div></div>`
      ).join('');
      const tbody = document.getElementById('rows');
      tbody.innerHTML = (results || []).map(r =>
        `<tr><td>${r.frame_id}</td><td class="${r.verdict === 'OK' ? 'ok' : 'ng'}">${r.verdict}</td>` +
        `<td>${r.defect_count}</td><td>${r.recipe_version}</td><td>${r.timestamp_ns}</td></tr>`
      ).join('');
    }
    async function applyThreshold() {
      const threshold = Number(document.getElementById('threshold').value);
      await fetch('/profile/vision/threshold', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ threshold }),
      });
      refresh();
    }
    refresh();
    setInterval(refresh, 2000);
  </script>
</body>
</html>"#,
    )
}
