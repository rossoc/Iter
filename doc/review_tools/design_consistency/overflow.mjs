import puppeteer from 'puppeteer-core';
const browser = await puppeteer.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
for (const url of process.argv.slice(2)) {
  const page = await browser.newPage();
  await page.setViewport({ width: 360, height: 800 });
  await page.goto(url, { waitUntil: 'networkidle0' });
  const r = await page.evaluate(() => {
    const vw = window.innerWidth;
    const wide = [];
    for (const e of document.querySelectorAll('body *')) {
      const b = e.getBoundingClientRect();
      if (b.right > vw + 1 && b.width > 0) wide.push(`${e.tagName}.${(e.className||'').toString().slice(0,40)} right=${Math.round(b.right)} w=${Math.round(b.width)}`);
    }
    return { scrollW: document.scrollingElement.scrollWidth, vw, wide: wide.filter(s => !s.startsWith("path") && !s.startsWith("svg") && !s.startsWith("circle")).slice(0, 25) };
  });
  console.log(url, JSON.stringify(r, null, 1));
  await page.close();
}
await browser.close();
