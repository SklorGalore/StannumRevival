import {test,expect,type Page} from '@playwright/test';
import {readFile} from 'node:fs/promises';

function rawCase(merged=false) {
  const sections=['GENERAL, THRSHZ=.0002',
    "1,'North',230,1,1,10,1,.99\n2,'South',115,1,2,20,1,1.02\n3,'West',230,3,1,20,1,1.04\n4,'Offline',115,4,2,10,1,1",
    '','','',`1,2,'A',${merged?'0,.0001':'.01,.1'}\n2,3,'B',.01,.1`];
  return Buffer.from('0,100,35,0,0,60\nField views\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n');
}

async function openCase(page:Page,name='fields.raw',merged=false) {
  await page.getByLabel('Open RAW file').setInputFiles({name,mimeType:'text/plain',buffer:rawCase(merged)});
  await expect(page.getByTestId('bus-count')).toHaveText('4');
}

async function addFilter(page:Page,target:string,field:string,operator:string,value:string) {
  await page.getByLabel('Filter target',{exact:true}).selectOption(target);
  await page.getByLabel('Filter field',{exact:true}).selectOption(field);
  await page.getByLabel('Filter operator',{exact:true}).selectOption(operator);
  await page.getByLabel('Filter value',{exact:true}).fill(value);
  await page.getByRole('button',{name:'Add filter',exact:true}).click();
}

test('field filters combine across graph, rows, and complete CSV exports while topology stays intact',async({page})=>{
  await page.goto('/');
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await addFilter(page,'bus','base_kv','gte','200');
  await expect(page.getByTestId('graph-status')).toContainText('2 nodes · 0 connections');
  await expect(page.locator('#table-container tbody tr')).toHaveCount(2);
  await expect(page.locator('.metric-accent p')).toContainText('1 islands');
  await page.getByLabel('Filter field',{exact:true}).selectOption('number');
  await page.getByLabel('Filter operator',{exact:true}).selectOption('gte');
  await page.getByLabel('Filter value',{exact:true}).fill('3');
  await page.getByRole('button',{name:'Add filter',exact:true}).click();
  await expect(page.locator('#table-container tbody tr')).toHaveCount(1);
  await expect(page.getByRole('button',{name:'Inspect bus 3',exact:true})).toBeVisible();
  const downloading=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export CSV',exact:true}).click();
  const csv=await readFile((await (await downloading).path())!,'utf8');
  expect(csv).toContain('3,West,230');
  expect(csv).not.toContain('1,North');
  await page.getByLabel('From bus').fill('1');
  await page.getByLabel('To bus').fill('3');
  await page.getByRole('button',{name:'Find path',exact:true}).click();
  await expect(page.getByTestId('path-result')).toContainText('2 equipment hops');
  await expect(page.getByTestId('field-view-note')).toContainText('Filters hide part of this path');
  await page.getByRole('button',{name:'Clear filters',exact:true}).click();
  await expect(page.getByTestId('graph-status')).toContainText('3 nodes · 2 connections');
});

test('equipment rules filter connections and equipment rows without suppressing bus results',async({page})=>{
  await page.goto('/');
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await addFilter(page,'equipment','circuit','contains','B');
  await expect(page.getByTestId('graph-status')).toContainText('3 nodes · 1 connections');
  await expect(page.locator('#table-container tbody tr')).toHaveCount(4);
  await page.locator('.nav-item[data-tab="equipment"]').click();
  await expect(page.locator('#table-container tbody tr')).toHaveCount(1);
  await expect(page.locator('#table-container tbody tr')).toContainText('line:2:3:B');
  await openCase(page,'replacement.raw');
  await expect(page.getByTestId('graph-status')).toContainText('3 nodes · 2 connections');
  await expect(page.getByRole('button',{name:'Clear filters',exact:true})).toBeDisabled();
});

test('custom category colors survive theme changes and reload without saving case contents',async({page})=>{
  await page.goto('/');
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await page.getByLabel('Color buses by',{exact:true}).selectOption('area');
  await page.getByLabel('Bus color category',{exact:true}).selectOption('1');
  await page.getByLabel('Bus category color',{exact:true}).fill('#e11d48');
  await page.getByRole('button',{name:'Apply bus category color',exact:true}).click();
  const swatch=page.locator('.field-color-legend [data-color-value="1"] i').first();
  await expect(swatch).toHaveCSS('background-color','rgb(225, 29, 72)');
  await page.getByRole('button',{name:'Dark theme',exact:true}).click();
  await expect(swatch).toHaveCSS('background-color','rgb(225, 29, 72)');
  const saved=await page.evaluate(()=>localStorage.getItem('nodal-field-colors'));
  expect(saved).toContain('#e11d48');
  expect(saved).not.toContain('North');
  expect(saved).not.toContain('raw_lines');
  await page.reload();
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await expect(page.getByLabel('Color buses by',{exact:true})).toHaveValue('area');
  await expect(swatch).toHaveCSS('background-color','rgb(225, 29, 72)');
  await page.getByRole('button',{name:'Reset bus colors',exact:true}).click();
  await expect(page.getByLabel('Color buses by',{exact:true})).toHaveValue('');
});

test('merged nodes disclose mixed colors and filters inspect original members',async({page})=>{
  await page.goto('/');
  await page.getByLabel('Treat zero-impedance branches as the same node').check();
  await openCase(page,'merged.raw',true);
  await page.getByText('Filters & colors',{exact:true}).click();
  await page.getByLabel('Color buses by',{exact:true}).selectOption('area');
  await expect(page.locator('.field-color-legend')).toContainText('Mixed');
  await addFilter(page,'bus','name','contains','South');
  await expect(page.getByTestId('graph-status')).toContainText('1 nodes · 0 connections');
  await expect(page.locator('#table-container tbody tr')).toHaveCount(1);
  await expect(page.getByRole('button',{name:'Inspect bus 1',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Inspect bus 1',exact:true}).click();
  await expect(page.locator('#inspector')).toContainText('South');
});

test('numeric palettes expose bounds and controls fit a narrow screen in both themes',async({page})=>{
  await page.goto('/');
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await page.getByLabel('Color buses by',{exact:true}).selectOption('voltage_pu');
  await page.getByLabel('Bus low color',{exact:true}).fill('#0000ff');
  await page.getByLabel('Bus high color',{exact:true}).fill('#ff0000');
  await page.getByLabel('Bus minimum',{exact:true}).fill('0.95');
  await page.getByLabel('Bus minimum',{exact:true}).press('Tab');
  await page.getByLabel('Bus maximum',{exact:true}).fill('1.05');
  await page.getByLabel('Bus maximum',{exact:true}).press('Tab');
  await expect(page.locator('.field-color-legend')).toContainText('0.95');
  await expect(page.locator('.field-color-legend')).toContainText('1.05');
  await page.screenshot({path:'test-results/field-colors-desktop.png',fullPage:true});
  await page.setViewportSize({width:320,height:800});
  await page.getByRole('button',{name:'Fit view',exact:true}).click();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.getByRole('button',{name:'Dark theme',exact:true}).click();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.screenshot({path:'test-results/field-colors-mobile-dark.png',fullPage:true});
});

test('filter editor preserves its field and category through color edits and view changes',async({page})=>{
  await page.goto('/');
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await page.getByLabel('Filter target',{exact:true}).selectOption('equipment');
  await page.getByLabel('Filter field',{exact:true}).selectOption('topology');
  await page.getByLabel('Filter operator',{exact:true}).selectOption('eq');
  await page.getByLabel('Filter value',{exact:true}).selectOption('Active');
  await page.getByLabel('Color buses by',{exact:true}).selectOption('area');
  await expect(page.getByLabel('Filter target',{exact:true})).toHaveValue('equipment');
  await expect(page.getByLabel('Filter field',{exact:true})).toHaveValue('topology');
  await expect(page.getByLabel('Filter value',{exact:true})).toHaveValue('Active');
  await page.locator('.nav-item[data-tab="equipment"]').click();
  await expect(page.getByLabel('Filter target',{exact:true})).toHaveValue('equipment');
  await expect(page.getByLabel('Filter field',{exact:true})).toHaveValue('topology');
  await page.getByRole('button',{name:'Add filter',exact:true}).click();
  await expect(page.locator('#table-container tbody tr')).toHaveCount(2);
});

test('filters rescue a capped graph and exports include matches beyond the current page',async({page})=>{
  const buses=Array.from({length:520},(_,i)=>`${i+1},'Bus ${i+1}',138,1`).join('\n');
  const lines=Array.from({length:519},(_,i)=>`${i+1},${i+2},'1',.01,.1`).join('\n');
  const sections=['',buses,'','','',lines];
  const raw=Buffer.from('0,100,35,0,0,60\nLarge field view\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n');
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({name:'large.raw',mimeType:'text/plain',buffer:raw});
  await expect(page.getByTestId('bus-count')).toHaveText('520');
  await page.getByRole('button',{name:'Show full network',exact:true}).click();
  await expect(page.getByTestId('graph-status')).toHaveText('No graph selected');
  await page.getByText('Filters & colors',{exact:true}).click();
  await addFilter(page,'bus','number','lte','40');
  await expect(page.getByTestId('graph-status')).toContainText('40 nodes · 39 connections');
  await expect(page.locator('#table-container tbody tr')).toHaveCount(25);
  const downloading=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export CSV',exact:true}).click();
  const csv=await readFile((await (await downloading).path())!,'utf8');
  expect(csv.trim().split('\r\n')).toHaveLength(41);
  expect(csv).toContain('40,Bus 40,138');
  expect(csv).not.toContain('41,Bus 41,138');
  const reporting=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export JSON',exact:true}).click();
  const report=JSON.parse(await readFile((await (await reporting).path())!,'utf8'));
  expect(report.graph.nodes).toHaveLength(520);
  expect(report.view.displayed_graph.nodes).toHaveLength(40);
  expect(report.view.filters[0]).toMatchObject({field:'number',operator:'lte',value:'40'});
  await page.getByRole('button',{name:'Clear filters',exact:true}).click();
  await expect(page.getByTestId('graph-status')).toHaveText('No graph selected');
});

test('text category customizations stay in session and category filters offer valid operators',async({page})=>{
  await page.goto('/');
  await openCase(page);
  await page.getByText('Filters & colors',{exact:true}).click();
  await page.getByLabel('Filter field',{exact:true}).selectOption('area');
  expect(await page.getByLabel('Filter operator',{exact:true}).locator('option').evaluateAll(options=>options.map(o=>(o as HTMLOptionElement).value))).not.toContain('contains');
  await page.getByLabel('Color buses by',{exact:true}).selectOption('name');
  await page.getByLabel('Bus color category',{exact:true}).selectOption('North');
  await page.getByLabel('Bus category color',{exact:true}).fill('#abcdef');
  await page.getByRole('button',{name:'Apply bus category color',exact:true}).click();
  await expect(page.locator('.field-color-legend [data-color-value="North"] i')).toHaveCSS('background-color','rgb(171, 205, 239)');
  const saved=await page.evaluate(()=>localStorage.getItem('nodal-field-colors'));
  expect(saved).not.toContain('North');
  await expect(page.getByText('Custom colors for names and circuits last for this session.')).toBeVisible();
  await page.getByLabel('Color equipment by',{exact:true}).selectOption('kind');
  await expect(page.locator('.field-color-legend')).toContainText('Equipment · Equipment type');
  await page.getByRole('button',{name:'Reset equipment colors',exact:true}).click();
  await expect(page.getByLabel('Color equipment by',{exact:true})).toHaveValue('');
});
