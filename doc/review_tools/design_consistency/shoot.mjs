// Screenshots every route in routes.json at 1280x900 and 360x800, light and dark
// (prefers-color-scheme emulated through puppeteer-core + the installed Chrome),
// and writes inventory.json with per-page facts read from the DOM.
import puppeteer from 'puppeteer-core';
import fs from 'node:fs';
import path from 'node:path';
const here = path.dirname(new URL(import.meta.url).pathname);
const routes = JSON.parse(fs.readFileSync(path.join(here, 'routes.json'), 'utf8'));
const out = path.join(here, 'screens');
fs.mkdirSync(out, { recursive: true });
const only = process.argv[2];
const browser = await puppeteer.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
  args: ['--hide-scrollbars', '--no-first-run'],
});
const inventory = [];
const facts = () => {
  const q = (s) => Array.from(document.querySelectorAll(s));
  const txt = (e) => (e ? e.textContent.replace(/\s+/g, ' ').trim() : null);
  const head = document.querySelector('.head');
  const kicker = head?.querySelector('.label') ? 'eyebrow:' + txt(head.querySelector('.label'))
    : head?.querySelector('nav[aria-label], .crumbs, .breadcrumb, ol') ? 'crumbs:' + txt(head.querySelector('nav[aria-label], .crumbs, .breadcrumb, ol')) : 'none';
  const tabsNav = q('nav').find((n) => /sections/i.test(n.getAttribute('aria-label') || ''));
  const buttons = q('.button, button').map((b) => (b.className + '|' + (txt(b) || b.getAttribute('aria-label') || '')).trim());
  const dialog = document.querySelector('[role=dialog]');
  return {
    title: document.title,
    h1: q('h1').map(txt),
    h1count: q('h1').length,
    kicker,
    tabs: tabsNav ? { label: tabsNav.getAttribute('aria-label'), items: q('a, span', ).filter(e => tabsNav.contains(e)).map(txt) } : null,
    nav: q('header.bar a, header.bar button').map(txt),
    headActions: q('.head-actions a, .head-actions button').map((b) => b.className + '|' + txt(b)),
    buttons,
    primary: q('.button.primary').map(txt),
    filterBar: !!document.querySelector('.filter-bar, form.filter, [class*=filter]'),
    footer: !!document.querySelector('footer.site-foot'),
    searchButton: !!document.querySelector('.search-button, [data-search], button[aria-keyshortcuts]'),
    empty: q('.empty-line, .empty p, tr.no-rows, .no-rows').map(txt),
    dialog: dialog ? { title: txt(dialog.querySelector('h2')), exits: q('a, button', ).filter(e => dialog.contains(e) && /close|cancel|back|✕/i.test(txt(e) || '') || (e.className||'').includes('modal-x')).map(e => e.className + '|' + txt(e)) } : null,
    inlineStyles: q('[style]').length,
    inlineStyleSamples: q('[style]').slice(0, 5).map((e) => e.tagName + '.' + e.className + '[' + e.getAttribute('style') + ']'),
    controls: { a: q('main a').length, button: q('main button').length, input: q('main input:not([type=hidden])').length, select: q('main select').length, textarea: q('main textarea').length, total: q('main a, main button, main input:not([type=hidden]), main select, main textarea').length, page: q('a, button, input:not([type=hidden]), select, textarea').length },
    overflowX: document.scrollingElement.scrollWidth - window.innerWidth,
    fontSizes: Array.from(new Set(q('main *').filter(e => e.children.length === 0 && txt(e)).map(e => getComputedStyle(e).fontSize))).sort((a,b)=>parseFloat(a)-parseFloat(b)),
    bodyBg: getComputedStyle(document.body).backgroundColor,
    unstyled: q('main button, main input, main select, main textarea').filter(e => !e.className && !e.closest('.form, form')).map(e => e.tagName + (e.name ? '[' + e.name + ']' : '')),
  };
};
for (const r of routes) {
  if (only && !r.name.includes(only)) continue;
  const rec = { name: r.name, url: r.url, shots: {} };
  for (const [w, h] of [[1280, 900], [360, 800]]) {
    for (const scheme of ['light', 'dark']) {
      const page = await browser.newPage();
      await page.setViewport({ width: w, height: h, deviceScaleFactor: 1 });
      await page.emulateMediaFeatures([{ name: 'prefers-color-scheme', value: scheme }]);
      const resp = await page.goto(r.url, { waitUntil: 'networkidle0', timeout: 20000 });
      rec.status = resp?.status();
      await new Promise((res) => setTimeout(res, 150));
      const file = path.join(out, `${r.name}_${w}_${scheme}.png`);
      await page.screenshot({ path: file, fullPage: true });
      rec.shots[`${w}_${scheme}`] = path.relative(here, file);
      if (scheme === 'light') rec[`facts_${w}`] = await page.evaluate(facts);
      await page.close();
    }
  }
  inventory.push(rec);
  console.log(rec.status, r.name, 'overflow360=', rec.facts_360?.overflowX, 'controls=', rec.facts_1280?.controls.total);
}
fs.writeFileSync(path.join(here, only ? `inventory_${only}.json` : 'inventory.json'), JSON.stringify(inventory, null, 1));
await browser.close();
