//! Minimal AOI dashboard — polls /stats, /results/recent, /spc/trend.

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
    h2 { font-size: 1rem; margin: 1.25rem 0 0.5rem; opacity: 0.85; }
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
    canvas { width: 100%; max-width: 640px; height: 120px; background: #1a2332; border-radius: 8px; }
    .chart-row { display: flex; gap: 1rem; flex-wrap: wrap; }
    .chart-box { flex: 1; min-width: 280px; }
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
  <h2>SPC trend</h2>
  <div class="chart-row">
    <div class="chart-box"><div class="label">NG rate</div><canvas id="ngChart" width="640" height="120"></canvas></div>
    <div class="chart-box"><div class="label">Gray mean</div><canvas id="grayChart" width="640" height="120"></canvas></div>
  </div>
  <table>
    <thead><tr><th>Frame</th><th>Verdict</th><th>Defects</th><th>Recipe</th><th>Time (ns)</th></tr></thead>
    <tbody id="rows"></tbody>
  </table>
  <script>
    function metricSeries(trend, name) {
      return (trend || []).map(s => {
        const v = (s.values || []).find(x => x.name === name);
        return v ? v.value : null;
      }).filter(v => v !== null);
    }
    function drawSparkline(canvasId, values, color) {
      const c = document.getElementById(canvasId);
      if (!c) return;
      const ctx = c.getContext('2d');
      ctx.clearRect(0, 0, c.width, c.height);
      if (!values.length) {
        ctx.fillStyle = '#556';
        ctx.fillText('no data', 12, 60);
        return;
      }
      const pad = 8;
      const w = c.width - pad * 2;
      const h = c.height - pad * 2;
      const min = Math.min(...values);
      const max = Math.max(...values);
      const span = max - min || 1;
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.beginPath();
      values.forEach((v, i) => {
        const x = pad + (i / Math.max(values.length - 1, 1)) * w;
        const y = pad + h - ((v - min) / span) * h;
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      });
      ctx.stroke();
    }
    async function refresh() {
      const [stats, results, profile, spc, trend] = await Promise.all([
        fetch('/stats').then(r => r.json()),
        fetch('/results/recent').then(r => r.json()),
        fetch('/profile').then(r => r.json()).catch(() => null),
        fetch('/spc/metrics').then(r => r.json()).catch(() => null),
        fetch('/spc/trend?limit=64').then(r => r.json()).catch(() => []),
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
      drawSparkline('ngChart', metricSeries(trend, 'ng_rate'), '#ff6b6b');
      drawSparkline('grayChart', metricSeries(trend, 'gray_mean'), '#3dd68c');
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
