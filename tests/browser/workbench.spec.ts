import { test, expect, type Page } from '@playwright/test';

function rawCase(name: string, buses: number, options: { excluded?: number | 'all'; longName?: string } = {}) {
  const busLines = Array.from({ length: buses }, (_, index) => {
    const number = index + 1;
    const busName = options.longName && number === 1 ? options.longName : `Bus ${number}`;
    const type = options.excluded === 'all' || number === (options.excluded ?? -1) ? 4 : 1;
    return `${number},'${busName}',138,${type}`;
  }).join('\n');
  const lineLines = Array.from({ length: Math.max(0, buses - 1) }, (_, index) =>
    `${index + 1},${index + 2},'1',1,.1`).join('\n');
  const sections = ['GENERAL, THRSHZ=.0002', busLines, '', '', '', lineLines, '', '', "'DC',1,0"];
  const content = `0,100,35,0,0,60\n${name}\n\n${sections.map(s => `${s}${s ? '\n' : ''}0\n`).join('')}0\n`;
  return Buffer.from(content + '0\n'.repeat(21 - sections.length) + 'Q\n');
}

async function importRaw(page: Page, name = 'workbench.raw', buses = 30, options: { excluded?: number | 'all'; longName?: string } = {}) {
  await page.getByLabel('Open RAW file').setInputFiles({
    name,
    mimeType: 'text/plain',
    buffer: rawCase(name, buses, options),
  });
  await expect(page.getByTestId('bus-count')).toHaveText(String(buses));
}

async function assertVerticalOrder(page: Page, selectors: string[]) {
  const tops = await Promise.all(selectors.map(selector => page.locator(selector).evaluate(el => el.getBoundingClientRect().top)));
  for (let index = 1; index < tops.length; index++) expect(tops[index]).toBeGreaterThan(tops[index - 1]);
}

async function captureScreenshot(page: Page, path: string) {
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path, fullPage: true });
}

test('analysis views stay disabled until a case loads successfully', async ({ page }) => {
  await page.goto('/');
  const views = page.getByRole('navigation', { name: 'Analysis views' }).getByRole('button');
  await expect(views).toHaveCount(6);
  for (const view of await views.all()) await expect(view).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Choose RAW file' })).toBeEnabled();
  await expect(page.getByLabel('Treat zero-impedance branches as the same node')).toBeEnabled();
  await page.locator('[data-tab="buses"]').dispatchEvent('click');
  await expect(page.locator('#view-title')).toHaveText('Overview');
  await page.getByLabel('Open RAW file').setInputFiles({ name: 'invalid.raw', mimeType: 'text/plain', buffer: Buffer.from('invalid') });
  await expect(page.getByRole('alert')).toBeVisible();
  for (const view of await views.all()) await expect(view).toBeDisabled();
  await importRaw(page);
  for (const view of await views.all()) await expect(view).toBeEnabled();
  await page.locator('[data-tab="buses"]').click();
  await expect(page.locator('#view-title')).toHaveText('Buses');
});

test('workbench uses readable typography, navigation semantics, and the light palette', async ({ page }) => {
  await page.goto('/');
  await importRaw(page);
  await expect(page.locator('[data-tab="overview"]')).toHaveAttribute('aria-current', 'page');

  const sizes = await page.evaluate(() => {
    const fontSize = (selector: string) => Number.parseFloat(getComputedStyle(document.querySelector(selector)!).fontSize);
    return {
      cell: fontSize('#table-container td'),
      input: fontSize('#result-search'),
      button: fontSize('.report-actions button'),
      secondary: fontSize('.case-meta'),
      workspace: getComputedStyle(document.documentElement).backgroundColor,
      surface: getComputedStyle(document.querySelector('.network-panel')!).backgroundColor,
      navigation: getComputedStyle(document.querySelector('.rail')!).backgroundColor,
    };
  });
  expect(sizes.cell).toBeGreaterThanOrEqual(14);
  expect(sizes.input).toBeGreaterThanOrEqual(14);
  expect(sizes.button).toBeGreaterThanOrEqual(14);
  expect(sizes.secondary).toBeGreaterThanOrEqual(12);
  expect(sizes.workspace).toBe('rgb(243, 245, 247)');
  expect(sizes.surface).toBe('rgb(255, 255, 255)');
  expect(sizes.navigation).toBe('rgb(20, 43, 60)');
});

test('graph leads results beside the inspector and narrow layouts stack in reading order', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('/');
  await importRaw(page);
  const desktop = await page.evaluate(() => {
    const rect = (selector: string) => {
      const r = document.querySelector(selector)!.getBoundingClientRect();
      return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
    };
    return { graph: rect('.network-panel'), results: rect('.results-panel'), inspector: rect('#inspector'), path: rect('.path-panel') };
  });
  expect(desktop.results.top).toBeGreaterThanOrEqual(desktop.graph.bottom - 1);
  expect(Math.abs(desktop.results.left - desktop.graph.left)).toBeLessThanOrEqual(1);
  expect(Math.abs(desktop.results.right - desktop.graph.right)).toBeLessThanOrEqual(1);
  expect(desktop.inspector.left).toBeGreaterThan(desktop.graph.right);
  expect(desktop.path.top).toBeGreaterThan(desktop.inspector.bottom - 1);
  await captureScreenshot(page, 'test-results/workbench-desktop.png');

  await page.reload();
  await captureScreenshot(page, 'test-results/workbench-import.png');
  await importRaw(page);

  await page.setViewportSize({ width: 800, height: 1000 });
  await assertVerticalOrder(page, ['.network-panel', '#inspector', '.results-panel', '.path-panel']);
  await captureScreenshot(page, 'test-results/workbench-narrow.png');

  await page.setViewportSize({ width: 720, height: 1000 });
  const zoomWidths = await page.evaluate(() => ({ viewport: innerWidth, document: document.documentElement.scrollWidth }));
  expect(zoomWidths.document).toBeLessThanOrEqual(zoomWidths.viewport);
  await captureScreenshot(page, 'test-results/workbench-desktop-200-percent-equivalent.png');

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByLabel('From bus')).toBeVisible();
  const widths = await page.evaluate(() => ({ viewport: innerWidth, document: document.documentElement.scrollWidth }));
  expect(widths.document).toBeLessThanOrEqual(widths.viewport);
});

test('long case names and source values stay within the workspace', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const long = `Very long bus/source identifier ${'ALPHA-1234567890-'.repeat(18)}`;
  await page.goto('/');
  await importRaw(page, `${'long-case-name-'.repeat(12)}.raw`, 3, { longName: long });
  await page.getByRole('button', { name: 'Inspect bus 1', exact: true }).click();
  await page.getByText(/^RAW fields/).click();
  const widths = await page.evaluate(() => ({ viewport: innerWidth, document: document.documentElement.scrollWidth }));
  expect(widths.document).toBeLessThanOrEqual(widths.viewport);
  await expect(page.locator('.source-details')).toBeVisible();
  await captureScreenshot(page, 'test-results/workbench-long-values.png');
});

test('inspection preserves search, page, endpoints, and graph scope', async ({ page }) => {
  await page.goto('/');
  await importRaw(page, 'context.raw', 30);
  await page.getByRole('button', { name: 'Show neighborhood' }).click();
  await page.getByLabel('From bus').fill('2');
  await page.getByLabel('To bus').fill('3');
  const graphTitle = page.locator('.network-panel h2');
  await expect(graphTitle).toHaveText('Bus 1 · 2-hop neighborhood');
  const graphStatus = await page.getByTestId('graph-status').textContent();
  await page.getByLabel('Search buses').fill('Bus');
  await page.getByRole('button', { name: 'Next' }).click();
  const searchValue = await page.getByLabel('Search buses').inputValue();
  const pageText = await page.locator('.pagination').innerText();
  const target = page.locator('#table-container [data-action="inspect-bus"]').first();
  const targetName = await target.getAttribute('aria-label');
  await target.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: /^Bus \d+$/ })).toBeVisible();
  await expect(page.getByLabel('Search buses')).toHaveValue(searchValue);
  await expect(page.locator('.pagination')).toContainText(pageText.split('\n')[0]);
  await expect(page.locator('.network-panel h2')).toHaveText('Bus 1 · 2-hop neighborhood');
  await expect(page.getByTestId('graph-status')).toHaveText(graphStatus!);
  await expect(page.getByLabel('From bus')).toHaveValue('2');
  await expect(page.getByLabel('To bus')).toHaveValue('3');
  const targetBus = await target.getAttribute('data-bus');
  await expect(page.getByRole('heading', { name: `Bus ${targetBus}`, exact: true })).toBeVisible();
  const selectedRow = page.locator('#table-container tr').filter({ has: page.locator(`[aria-label="${targetName}"]`) });
  await expect(selectedRow).toHaveAttribute('data-selected', 'true');
  await expect(selectedRow).toContainText('Selected');
});

test('merged and excluded source buses remain inspectable without expanding graph scope', async ({ page }) => {
  const source = rawCase('source.raw', 4, { excluded: 4 });
  const withZeroTie = source.toString().replace("1,2,'1',1,.1", "1,2,'1',0,.0001");
  await page.goto('/');
  await page.locator('#drop-zone').getByRole('checkbox', { name: 'Treat zero-impedance branches as the same node' }).check();
  await page.getByLabel('Open RAW file').setInputFiles({ name: 'merged.raw', mimeType: 'text/plain', buffer: Buffer.from(withZeroTie) });
  await expect(page.getByTestId('bus-count')).toHaveText('4');
  const before = await page.getByTestId('graph-status').textContent();
  await page.getByRole('button', { name: 'Inspect bus 1', exact: true }).click();
  await expect(page.getByRole('heading', { name: /Merged node 1/ })).toBeVisible();
  await page.getByLabel('Search buses').fill('Bus 2');
  await expect(page.getByRole('button', { name: 'Inspect bus 1', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Inspect bus 1', exact: true }).click();
  await expect(page.locator('#inspector')).toContainText('Merged node 1');
  await expect(page.locator('.legend')).toContainText('Selected');
  await page.locator('[data-action="source-bus"][data-bus="2"]').click();
  await expect(page.getByRole('heading', { name: 'Bus 2', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Back to merged node' }).click();
  await expect(page.getByRole('heading', { name: /Merged node 1/ })).toBeVisible();
  await expect(page.getByTestId('graph-status')).toHaveText(before!);
  await page.getByLabel('Search buses').fill('Bus 4');
  await expect(page.getByRole('button', { name: 'Inspect bus 4', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Inspect bus 4', exact: true }).click();
  await expect(page.locator('#inspector')).toContainText('Excluded');
  await expect(page.getByRole('button', { name: 'Show neighborhood' })).toBeDisabled();
  await expect(page.getByTestId('graph-status')).toHaveText(before!);
  await expect(page.locator('.legend')).not.toContainText('Selected');
  await captureScreenshot(page, 'test-results/workbench-merged-inspector.png');
});

test('narrow workbench supports search, inspection, tracing, export, and internal table scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await importRaw(page, 'mobile-case.raw', 30);
  await page.getByLabel('Search buses').fill('Bus 2');
  await page.getByRole('button', { name: 'Inspect bus 2', exact: true }).click();
  await page.getByLabel('From bus').fill('1');
  await page.getByLabel('To bus').fill('3');
  await page.getByRole('button', { name: 'Find path' }).click();
  await expect(page.getByTestId('path-result')).toContainText('2 equipment hops');
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export JSON' }).click();
  await expect(downloadPromise).resolves.toBeTruthy();
  await assertVerticalOrder(page, ['.network-panel', '#inspector', '.results-panel', '.path-panel']);
  const dimensions = await page.evaluate(() => ({
    viewport: innerWidth,
    document: document.documentElement.scrollWidth,
    table: document.querySelector('.table-scroll')?.getBoundingClientRect().width ?? 0,
    tableContent: document.querySelector('.table-scroll')?.scrollWidth ?? 0,
    tableOverflow: getComputedStyle(document.querySelector('.table-scroll')!).overflowX,
  }));
  expect(dimensions.document).toBeLessThanOrEqual(dimensions.viewport);
  expect(dimensions.tableOverflow).toMatch(/auto|scroll/);
  expect(dimensions.tableContent).toBeGreaterThan(dimensions.table);
  await captureScreenshot(page, 'test-results/workbench-mobile-investigation.png');
});

test('selected path bridge and articulation remain visible together', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  await importRaw(page, 'overlap.raw', 5);
  await page.getByLabel('From bus').fill('1');
  await page.getByLabel('To bus').fill('5');
  await page.getByRole('button', { name: 'Find path' }).click();
  await expect(page.getByTestId('path-result')).toContainText('4 equipment hops');
  await page.getByRole('button', { name: 'Inspect bus 3', exact: true }).click();
  await expect(page.locator('.legend')).toContainText('Path');
  await expect(page.locator('.legend')).toContainText('Topology bridge');
  await expect(page.locator('.legend')).toContainText('Articulation bus');
  await expect(page.locator('.legend')).toContainText('Selected');
  await expect(page.locator('#network-canvas canvas').first()).toBeVisible();
  await expect.poll(() => page.locator('#network-canvas canvas').count()).toBeGreaterThan(0);
  const paintedPixels = async () => page.locator('#network-canvas canvas').evaluateAll(canvases => canvases.reduce((total, canvas) => {
    const context = canvas.getContext('2d');
    if (!context || canvas.width === 0 || canvas.height === 0) return total;
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let painted = 0;
    for (let index = 3; index < pixels.length; index += 4) if (pixels[index] > 0) painted++;
    return total + painted;
  }, 0));
  await expect.poll(paintedPixels).toBeGreaterThan(100);
  expect(errors).toEqual([]);
  await captureScreenshot(page, 'test-results/workbench-overlapping-graph-states.png');
});

test('keyboard inspection keeps a visible focus and reduced motion keeps loading feedback', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      set onmessage(handler: ((this: Worker, event: MessageEvent) => unknown) | null) {
        super.onmessage = event => {
          if (event.data.type === 'loaded') setTimeout(() => handler?.call(this, event), 400);
          else handler?.call(this, event);
        };
      }
    };
  });
  await page.goto('/');
  const chooser = page.getByRole('button', { name: 'Choose RAW file' });
  await chooser.focus();
  await expect(chooser).toBeFocused();
  const outline = await chooser.evaluate(el => getComputedStyle(el).outlineStyle);
  expect(outline).not.toBe('none');
  const fileChooserPromise = page.waitForEvent('filechooser');
  await page.keyboard.press('Enter');
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.getByText(/Building the topology|Rebuilding topology/)).toBeVisible();
  const reducedDuration = await page.locator('.progress-track span').evaluate(el => getComputedStyle(el).animationDuration).catch(() => '0s');
  expect(reducedDuration.split(',').every(duration => Number.parseFloat(duration) === 0)).toBeTruthy();
  await expect(page.getByTestId('bus-count')).toHaveText('993');
  const islands = page.getByRole('button', { name: /Islands/ });
  await islands.focus();
  await page.keyboard.press('Enter');
  await expect(islands).toHaveAttribute('aria-current', 'page');
  await expect(islands).toBeFocused();
  const overview = page.getByRole('button', { name: 'Overview', exact: false });
  await overview.focus();
  await page.keyboard.press('Enter');
  await expect(overview).toHaveAttribute('aria-current', 'page');
  await page.getByLabel('Search buses').fill('Greers');
  const inspect = page.getByRole('button', { name: 'Inspect bus 2', exact: true });
  await inspect.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Bus 2', exact: true })).toBeVisible();
  await expect(inspect).toBeFocused();
  const rawSummary = page.locator('#inspector summary').filter({ hasText: 'RAW fields' });
  await rawSummary.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('#inspector details.source-details')).toHaveAttribute('open', '');
  const from = page.getByLabel('From bus');
  await from.fill('2');
  await page.getByLabel('To bus').fill('3');
  const findPath = page.getByRole('button', { name: 'Find path' });
  await findPath.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('path-result')).toContainText('1 equipment hop');
  await expect(page.getByLabel('From bus')).toHaveValue('2');
  await expect(page.getByLabel('To bus')).toHaveValue('3');
  await expect(findPath).toBeFocused();
});

test('no-match and no-active-topology states stay readable', async ({ page }) => {
  await page.goto('/');
  await importRaw(page, 'empty-search.raw', 3);
  await page.getByLabel('Search buses').fill('nothing matches this case');
  await expect(page.getByText('No results match your search.')).toBeVisible();
  await importRaw(page, 'excluded.raw', 3, { excluded: 'all' });
  await expect(page.getByText('No graph selected')).toBeVisible();
  await expect(page.locator('#inspector')).toContainText('Inspect a bus or connection');
});
