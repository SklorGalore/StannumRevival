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

test('reports desktop save failures without losing the case and ignores stale failures after replacement', async ({page})=>{
  await page.addInitScript(()=>{
    let saves=0;
    Object.assign(window,{
      isTauri:true,
      __TAURI_INTERNALS__:{
        invoke:(command:string)=>{
          if(command!=='save_export') throw new Error(`Unexpected native command: ${command}`);
          saves++;
          if(saves===1) return Promise.reject('Permission denied while saving');
          window.dispatchEvent(new Event('desktop-save-export-started'));
          return new Promise((_,reject)=>setTimeout(()=>reject('Stale save failure'),1000));
        },
      },
    });
  });
  const rawSections=['',"1,'Origin',138,1\n2,'North',138,1",'','','',"1,2,'1',1,.1",'','','\'DC\',1,0'];
  const raw='0,100,35,0,0,60\nDesktop save test\n\n'+rawSections.map(section=>section+(section?'\n':'')+'0\n').join('')+'0\n'.repeat(21-rawSections.length)+'Q\n';
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({name:'tiny.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.getByTestId('bus-count')).toHaveText('2');

  await page.getByRole('button',{name:'Export JSON'}).click();
  await expect(page.locator('#workspace .error-banner strong')).toHaveText('EXPORT');
  await expect(page.locator('#workspace .error-banner')).toContainText('Permission denied while saving');
  await expect(page.getByTestId('bus-count')).toHaveText('2');
  await page.getByLabel('From bus').fill('1');
  await page.getByLabel('To bus').fill('2');
  await page.getByRole('button',{name:'Find path'}).click();
  await expect(page.getByTestId('path-result')).toContainText('1 equipment hop');

  const saveStarted=page.evaluate(()=>new Promise<void>(resolve=>window.addEventListener('desktop-save-export-started',()=>resolve(),{once:true})));
  await page.getByRole('button',{name:'Export JSON'}).click();
  await saveStarted;
  await page.getByLabel('Open RAW file').setInputFiles('Texas2k_series25_case1_summerpeak.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('2,751');
  await page.waitForTimeout(1100);
  await expect(page.locator('#workspace .error-banner')).toHaveCount(0);
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

test('exports every active bus within the inspected bus equipment-hop distance', async ({page})=>{
  const sections=['',
    "1,'Origin',138,1\n2,'One hop',138,1\n3,'Two hops',138,1\n4,'Three hops',138,1\n5,'Disconnected',138,4",
    '', '', '', '',
    "1,2,'1',1,.1\n2,3,'1',1,.1\n3,4,'1',1,.1",
    '', '', "'DC',1,0"];
  const raw='0,100,35,0,0,60\nTiny chain\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n';
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({name:'chain.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.getByTestId('bus-count')).toHaveText('5');
  await page.getByRole('button',{name:'Inspect bus 1',exact:true}).click();
  const distance=page.getByLabel('Equipment hops');
  await expect(distance).toHaveValue('2');

  const exportCsv=async(distanceValue:string)=>{
    await distance.fill(distanceValue);
    const downloading=page.waitForEvent('download');
    await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
    const download=await downloading;
    expect(download.suggestedFilename()).toBe(`chain-bus-1-${distanceValue}-hops.csv`);
    return readFile((await download.path())!,'utf8');
  };
  const columns='bus,name,base_kv,degree,island,status,articulation';
  expect(await exportCsv('0')).toBe(`${columns}\r\n1,Origin,138,1,1,Active,false\r\n`);
  expect(await exportCsv('1')).toBe(`${columns}\r\n1,Origin,138,1,1,Active,false\r\n2,One hop,138,2,1,Active,true\r\n`);
  expect(await exportCsv('2')).toBe(`${columns}\r\n1,Origin,138,1,1,Active,false\r\n2,One hop,138,2,1,Active,true\r\n3,Two hops,138,2,1,Active,true\r\n`);
  await distance.fill('1');
  await page.getByRole('button',{name:'Show neighborhood'}).click();
  await expect(page.getByRole('heading',{name:'Bus 1 · 1-hop neighborhood'})).toBeVisible();
  await expect(page.getByTestId('graph-status')).toContainText('2 nodes');
  await distance.fill('2');

  await page.getByLabel('Search buses').fill('Three hops');
  await page.getByRole('button',{name:'Islands',exact:false}).click();
  const downloading=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
  const download=await downloading;
  expect(await readFile((await download.path())!,'utf8')).toContain('3,Two hops');
  expect(await readFile((await download.path())!,'utf8')).not.toContain('4,Three hops');

  await distance.fill('-1');
  const downloads:string[]=[];
  page.on('download',item=>downloads.push(item.suggestedFilename()));
  await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
  await expect.poll(()=>distance.evaluate(input=>(input as HTMLInputElement).validity.valid)).toBe(false);
  await page.waitForTimeout(250);
  await distance.fill('1.5');
  await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
  await expect.poll(()=>distance.evaluate(input=>(input as HTMLInputElement).validity.valid)).toBe(false);
  await page.waitForTimeout(250);
  await distance.fill('');
  await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
  await expect.poll(()=>distance.evaluate(input=>(input as HTMLInputElement).validity.valid)).toBe(false);
  await page.waitForTimeout(250);
  expect(downloads).toEqual([]);

  await page.getByRole('button',{name:'Buses',exact:false}).click();
  await page.getByRole('button',{name:'Inspect bus 5',exact:true}).click();
  await expect(page.getByRole('button',{name:'Export neighborhood CSV'})).toBeDisabled();
});

test('neighborhood CSV exports beyond the graph display cap',async({page})=>{
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles('MemphisCase2026_Mar7.RAW');
  await expect(page.getByTestId('bus-count')).toHaveText('993');
  await page.getByRole('button',{name:'Inspect bus 2',exact:true}).click();
  await page.getByLabel('Equipment hops').fill('100000');
  await page.getByRole('button',{name:'Show neighborhood'}).click();
  await expect(page.getByText('Choose a bus neighborhood or path to stay within the 500-node display limit.',{exact:false})).toBeVisible();
  const downloading=page.waitForEvent('download');
  await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
  const download=await downloading;
  const csv=await readFile((await download.path())!,'utf8');
  expect(csv.split('\r\n').length-2).toBeGreaterThan(500);
  expect(csv).not.toContain('junction:');
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

test('global zero impedance topology applies before import and throughout views and exports',async({page})=>{
  const sections=['GENERAL, THRSHZ=.0002',
    "1,'Origin',138,1\n2,'Boundary',138,1\n3,'Chain end',138,1\n4,'Outside threshold',138,1",
    '', '', '',
    "1,2,'1',0,.0002\n2,3,'1',0,.0001\n3,4,'1',0,.0002001",
    '', '', "'DC',1,0"];
  const raw='0,100,35,0,0,60\nThreshold case\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n';
  await page.goto('/');
  const importMerge=page.locator('#drop-zone').getByRole('checkbox',{name:'Treat zero-impedance branches as the same node'});
  await expect(importMerge).not.toBeChecked();
  await importMerge.check();
  await page.getByLabel('Open RAW file').setInputFiles({name:'threshold.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.getByTestId('bus-count')).toHaveText('4');
  await expect(page.locator('.metric-accent strong')).toHaveText('2');
  const merge=page.locator('.report-actions').getByRole('checkbox',{name:'Treat zero-impedance branches as the same node'});
  await expect(merge).toBeChecked();
  await expect(page.getByTestId('graph-status')).toContainText('2 nodes');
  await page.getByLabel('Search buses').fill('Chain end');
  await expect(page.getByRole('button',{name:'Inspect bus 1',exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Inspect bus 3',exact:true})).toHaveCount(0);
  await page.getByLabel('Search buses').fill('2');
  await expect(page.getByRole('button',{name:'Inspect bus 1',exact:true})).toBeVisible();
  await page.getByLabel('Search buses').fill('');
  await page.getByRole('button',{name:'Inspect bus 1',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Merged node 1 · 3 buses'})).toBeVisible();
  await expect(page.getByText('Boundary',{exact:true})).toBeVisible();
  await page.locator('[data-action="source-bus"][data-bus="2"]').click();
  await expect(page.getByRole('heading',{name:'Bus 2',exact:true})).toBeVisible();
  await page.locator('#inspector [data-action="inspect-bus"]').click();
  await expect(page.getByRole('heading',{name:'Merged node 1 · 3 buses'})).toBeVisible();
  await expect(page.getByText('THRSHZ 0.0002 p.u.')).toBeVisible();
  const distance=page.getByLabel('Equipment hops');
  await distance.fill('0');
  const exportCsv=async()=>{
    const downloading=page.waitForEvent('download');
    await page.getByRole('button',{name:'Export neighborhood CSV'}).click();
    const download=await downloading;
    return {name:download.suggestedFilename(),csv:await readFile((await download.path())!,'utf8')};
  };
  const unchecked=await exportCsv();
  expect(unchecked.name).toBe('threshold-bus-1-0-hops.csv');
  expect(unchecked.csv).toContain('1,Origin');
  expect(unchecked.csv).not.toContain('2,Boundary');
  await merge.check();
  const grouped=await exportCsv();
  expect(grouped.name).toBe('threshold-bus-1-0-hops.csv');
  expect(grouped.csv).toContain('1,Origin,1 → 2 → 3,138');
  expect(grouped.csv).not.toContain('2,Boundary');
  expect(grouped.csv).not.toContain('4,Outside threshold');
  await distance.fill('1');
  const oneHop=await exportCsv();
  expect(oneHop.csv).toContain('4,Outside threshold');
  expect(oneHop.csv).toContain('1 → 2 → 3');
  await page.getByRole('button',{name:'Show neighborhood'}).click();
  await expect(page.getByRole('heading',{name:'Bus 1 · 1-hop neighborhood'})).toBeVisible();
  await expect(page.getByTestId('graph-status')).toContainText('2 nodes');
  await page.getByLabel('From bus').fill('2');await page.getByLabel('To bus').fill('3');
  await page.getByRole('button',{name:'Find path'}).click();
  await expect(page.getByTestId('path-result')).toContainText('0 equipment hops');
  const reportPromise=page.waitForEvent('download');await page.getByRole('button',{name:'Export JSON'}).click();
  const reportDownload=await reportPromise;const report=JSON.parse(await readFile((await reportDownload.path())!,'utf8'));
  expect(report.merge_zero_impedance).toBe(true);expect(report.graph.nodes.find((n:{bus:number})=>n.bus===1).member_buses).toEqual([1,2,3]);
  expect(report.case.buses).toHaveLength(4);
  await page.getByRole('button',{name:'Show full network'}).click();
  await expect(page.getByRole('heading',{name:'Full network'})).toBeVisible();
  await expect(page.getByTestId('graph-status')).toContainText('2 nodes');
  await merge.uncheck();
  await expect(page.locator('.metric-accent strong')).toHaveText('4');
  await page.getByRole('button',{name:'Critical connections',exact:false}).click();
  await expect(page.locator('#nav-critical')).toHaveText('3');
  await page.getByRole('button',{name:'Overview',exact:false}).click();
  await distance.fill('0');
  const restored=await exportCsv();
  expect(restored.csv).toContain('1,Origin');
  expect(restored.csv).not.toContain('2,Boundary');
  await merge.check();
  await merge.check();
  await page.getByLabel('Open RAW file').setInputFiles({name:'replacement.raw',mimeType:'text/plain',buffer:Buffer.from(raw.replace('GENERAL, THRSHZ=.0002','GENERAL, THRSHZ=0'))});
  await expect(page.locator('.report-actions').getByRole('checkbox',{name:'Treat zero-impedance branches as the same node'})).toBeChecked();
  await expect(page.getByTestId('graph-status')).toContainText('3 nodes');
  await expect(page.getByText('THRSHZ is 0, so no zero-impedance branches are grouped.')).toBeVisible();
  await merge.uncheck();
  await merge.check();
  await distance.fill('0');
  const disabledThreshold=await exportCsv();
  expect(disabledThreshold.csv).toContain('1,Origin');
  expect(disabledThreshold.csv).not.toContain('2,Boundary');
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

test('replacement case wins over delayed topology and path responses',async({page})=>{
  await page.addInitScript(()=>{
    const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker {
      topologyIds=new Set<number>();
      postMessage(message:any,transfer?:Transferable[]) {
        if(message.type==='topology') this.topologyIds.add(message.id);
        super.postMessage(message,transfer as Transferable[]);
      }
      set onmessage(handler:((this:Worker,event:MessageEvent)=>unknown)|null) {
        super.onmessage=(event:MessageEvent)=>{
          if(event.data.type==='loaded'&&this.topologyIds.delete(event.data.id)) setTimeout(()=>{handler?.call(this,event);window.dispatchEvent(new Event('delayed-topology-delivered'));},500);
          else if(event.data.type==='path') setTimeout(()=>{handler?.call(this,event);window.dispatchEvent(new Event('delayed-path-delivered'));},400);
          else handler?.call(this,event);
        };
      }
    };
  });
  const sections=['GENERAL, THRSHZ=.0002',"1,'Origin',138,1\n2,'Boundary',138,1\n3,'Chain end',138,1\n4,'Outside threshold',138,1",'','','',"1,2,'1',0,.0002\n2,3,'1',0,.0001\n3,4,'1',0,.0002001",'','','\'DC\',1,0'];
  const raw='0,100,35,0,0,60\nThreshold case\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n';
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({name:'first.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.getByTestId('bus-count')).toHaveText('4');
  await page.getByLabel('From bus').fill('1');await page.getByLabel('To bus').fill('4');
  await page.getByRole('button',{name:'Find path'}).click();
  const merge=page.locator('.report-actions').getByRole('checkbox',{name:'Treat zero-impedance branches as the same node'});
  await expect(merge).not.toBeChecked();
  await merge.click();
  await expect(merge).toBeDisabled();
  await expect(page.getByLabel('Equipment hops')).toBeDisabled();
  await expect(page.locator('#inspector [data-action="inspect-equipment"]').first()).toBeDisabled();
  await expect(page.locator('#table-container [data-action="inspect-bus"]').first()).toBeDisabled();
  await expect(page.getByRole('button',{name:'Find path'})).toBeDisabled();
  await page.getByLabel('Open RAW file').setInputFiles({name:'replacement.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.getByTestId('filename')).toHaveText('replacement.raw');
  await expect(merge).toBeChecked();
  await expect(page.getByTestId('graph-status')).toContainText('2 nodes');
  await page.waitForTimeout(600);
  await expect(page.getByTestId('filename')).toHaveText('replacement.raw');
  await expect(page.getByTestId('path-result')).toContainText('Choose two bus numbers');
});

test('topology rebuild keeps the selected island when island numbering changes',async({page})=>{
  const sections=['GENERAL, THRSHZ=.001',"1,'Merge A',138,1\n2,'Merge B',138,1\n3,'Merge C',138,1\n10,'Other A',138,1\n11,'Other B',138,1",'','','',"1,2,'1',0,.0001\n2,3,'1',0,.0001\n10,11,'1',.01,.1",'','','\'DC\',1,0'];
  const raw='0,100,35,0,0,60\nIsland order case\n\n'+sections.map(s=>s+(s?'\n':'')+'0\n').join('')+'0\n'.repeat(21-sections.length)+'Q\n';
  await page.goto('/');
  await page.getByLabel('Open RAW file').setInputFiles({name:'islands.raw',mimeType:'text/plain',buffer:Buffer.from(raw)});
  await expect(page.locator('.metric-accent p')).toContainText('2 islands');
  await page.getByRole('button',{name:'Islands',exact:false}).click();
  await expect(page.getByRole('cell',{name:'1',exact:true}).first()).toBeVisible();
  await page.getByRole('button',{name:'View island'}).first().click();
  await expect(page.getByRole('heading',{name:'Island 1'})).toBeVisible();
  const merge=page.locator('.report-actions').getByRole('checkbox',{name:'Treat zero-impedance branches as the same node'});
  await merge.click();
  await expect(merge).toBeChecked();
  await expect(page.getByRole('heading',{name:'Island 2'})).toBeVisible();
  await expect(page.getByTestId('graph-status')).toContainText('1 nodes');
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
