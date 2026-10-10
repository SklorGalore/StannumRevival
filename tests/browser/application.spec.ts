import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';

test('imports supplied cases privately and exports consistent topology results', async ({ page }) => {
  const uploads: string[] = [];
  const errors: string[] = [];
  page.on('request', r => { if (r.method() !== 'GET') uploads.push(r.url()); });
  page.on('pageerror', e => errors.push(e.message));
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('993');
  await expect(page.getByTestId('line-count')).toHaveText('975');
  await expect(page.getByTestId('transformer-count')).toHaveText('407');
  await page.getByLabel('Search buses').fill('Greers');
  await expect(page.getByRole('button', {name:'Inspect bus 2',exact:true})).toBeVisible();
  await page.getByRole('button', {name:'Inspect bus 2',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Bus 2',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Show neighborhood'}).click();
  await expect(page.getByTestId('graph-status')).toContainText('nodes');
  await page.getByRole('button',{name:'Islands',exact:false}).click();
  await page.getByRole('button',{name:'View island'}).first().click();
  await expect(page.getByText('Choose a bus neighborhood or path to stay within the 500-node display limit.',{exact:false})).toBeVisible();
  await page.getByLabel('From bus').fill('2');
  await page.getByLabel('To bus').fill('3');
  await page.getByRole('button',{name:'Find path'}).click();
  await expect(page.getByTestId('path-result')).toContainText('1 equipment hop');
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button',{name:'Export JSON'}).click();
  const download = await downloadPromise;
  const report = JSON.parse(await readFile((await download.path())!, 'utf8'));
  expect(report.case.buses).toHaveLength(993);
  expect(report.analysis.active_buses + report.analysis.excluded_buses).toBe(993);
  expect(report.path.equipment_ids).toEqual(['transformer:2:3:1']);
  await page.getByLabel('Open RAW file').setInputFiles('Texas2k_series25_case1_summerpeak.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('2,751');
  await expect(page.getByTestId('line-count')).toHaveText('3,993');
  await expect(page.getByTestId('transformer-count')).toHaveText('1,351');
  await page.screenshot({path:'test-results/texas-overview.png',fullPage:true});
  expect(uploads).toEqual([]);
  expect(errors).toEqual([]);
});

test('reports topology omissions and exports filtered CSV including quotes', async ({page})=>{
  const sections=['',"1,'<img src=x onerror=alert(1)>',138,1\n2,'A, B',138,1",'','','',"1,2,'1',0,.1",'','','',"'DC',1,0"];
  const raw='0,100,35,0,0,60\nTest\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n';
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({name:'tiny.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.getByText('Topology coverage is limited')).toBeVisible();
  await expect(page.getByTestId('bus-count')).toHaveText('2');
  await page.getByLabel('Search buses').fill('A, B');
  const downloading=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export CSV'}).click();
  const download=await downloading;
  const csv=await readFile((await download.path())!,'utf8');
  expect(csv).toContain('2,"A, B",138,1,1,Active,false');
  expect(csv).not.toContain('onerror');
  await page.getByLabel('From bus').fill('1');await page.getByLabel('To bus').fill('99');
  await page.getByRole('button',{name:'Find path'}).click();
  await expect(page.getByTestId('path-result')).toContainText('does not exist');
  await page.getByRole('button',{name:'Review diagnostics'}).click();
  await expect(page.getByRole('cell',{name:'TWO-TERMINAL DC',exact:true})).toBeVisible();
  expect(await page.locator('#inspector img').count()).toBe(0);
});

test('small-screen layout remains usable',async({page})=>{
  await page.setViewportSize({width:390,height:844});
  await page.goto('/');
  await expect(page.getByRole('button',{name:'Choose RAW file'})).toBeVisible();
  await page.getByLabel('Open RAW file').setInputFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('993');
  await expect(page.getByLabel('From bus')).toBeVisible();
  await page.screenshot({path:'test-results/mobile-overview.png',fullPage:true});
  const widths=await page.evaluate(()=>({viewport:innerWidth,width:document.documentElement.scrollWidth,overflow:[...document.querySelectorAll('body *')].map(el=>({element:el.tagName+':'+(el.className||el.id),width:el.getBoundingClientRect().width,right:el.getBoundingClientRect().right})).filter(el=>el.right>innerWidth+1).slice(0,30)}));
  expect(widths.width,JSON.stringify(widths)).toBeLessThanOrEqual(widths.viewport);
});

test('a newer neighborhood selection wins over an in-flight path',async({page})=>{
  await page.addInitScript(()=>{
    const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker {
      set onmessage(handler:((this:Worker,event:MessageEvent)=>unknown)|null) {
        super.onmessage=(event:MessageEvent)=>{
          if(event.data.type==='path') setTimeout(()=>{handler?.call(this,event);window.dispatchEvent(new Event('delayed-path-delivered'));},300);
          else handler?.call(this,event);
        };
      }
    };
  });
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('993');
  await page.getByLabel('From bus').fill('2');await page.getByLabel('To bus').fill('3');
  const delivered=page.evaluate(()=>new Promise<void>(resolve=>window.addEventListener('delayed-path-delivered',()=>resolve(),{once:true})));
  await page.getByRole('button',{name:'Find path'}).click();
  await page.getByRole('button',{name:'Show neighborhood'}).click();
  await delivered;
  await expect(page.getByRole('heading',{name:'Bus 2 · 2-hop neighborhood'})).toBeVisible();
  const downloading=page.waitForEvent('download');await page.getByRole('button',{name:'Export JSON'}).click();
  const download=await downloading;
  expect(JSON.parse(await readFile((await download.path())!,'utf8')).path).toBeNull();
  await expect(page.getByRole('button',{name:'Find path'})).toBeEnabled();
});

test('rejects invalid files and replaces an in-flight case without stale results', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({ name:'bad.raw',mimeType:'text/plain',buffer:Buffer.from('0,100,33\n\n\nQ') });
  await expect(page.getByRole('alert')).toContainText('revision 35');
  await page.getByLabel('Open RAW file').setInputFiles('Texas2k_series25_case1_summerpeak.RAW');
  await page.getByLabel('Open RAW file').setInputFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('993');
  await expect(page.getByTestId('filename')).toHaveText('MemphisCase2026_Mar7.RAW');
});
