import './style.css';
import {BrowserEngine} from './engine';
import {NetworkGraph} from './graph';
import {focusedGraph,neighborhood,toCsv} from './view-model';
import type {Snapshot,GraphData,Path,Diagnostic,SourceRecord} from './types';

const logo=`<svg viewBox="0 0 40 40" fill="none" aria-hidden="true"><path d="M10 11L29 10L21 29L10 11M10 11L7 30L21 29" stroke="currentColor" stroke-width="1.5"/><circle cx="10" cy="11" r="4" fill="currentColor"/><circle cx="29" cy="10" r="3" fill="currentColor"/><circle cx="21" cy="29" r="4" fill="currentColor"/><circle cx="7" cy="30" r="2" fill="currentColor"/></svg>`;
const tabs=[['overview','Overview','◈'],['islands','Islands','◌'],['buses','Buses','⊙'],['critical','Critical connections','◇'],['equipment','Equipment','⇄'],['diagnostics','Diagnostics','≡']] as const;
type Tab=typeof tabs[number][0];
const app=document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML=`
  <aside class="rail">
    <a class="brand" href="/">${logo}<span>nodal<span class="brand-sub">NETWORK EXPLORER</span></span></a>
    <div class="rail-label">WORKSPACE</div>
    <nav aria-label="Analysis views">${tabs.map(([id,label,icon])=>`<button data-action="tab" data-tab="${id}" class="nav-item ${id==='overview'?'active':''}"><span class="nav-icon" aria-hidden="true">${icon}</span>${label}<span class="nav-count" id="nav-${id}"></span></button>`).join('')}</nav>
    <div class="rail-bottom"><span class="local-dot"></span> Local browser workspace<p>Files stay on your device.<br>Powered by Rust + WebAssembly.</p><span class="version">NODAL / 0.1</span></div>
  </aside>
  <main>
    <header class="topbar"><div class="breadcrumb">Workspace <span>/</span> <strong id="view-title">Overview</strong></div><div class="top-actions"><span class="privacy"><span class="local-dot"></span> LOCAL ONLY</span><label class="button primary file-button" for="raw-file"><span aria-hidden="true">＋</span> Open RAW file</label><input id="raw-file" type="file" accept=".raw,.RAW" aria-label="Open RAW file" class="file-input" /></div></header>
    <section id="workspace" aria-busy="false"></section>
  </main>`;

const workspace=document.querySelector<HTMLElement>('#workspace')!;
const fileInput=document.querySelector<HTMLInputElement>('#raw-file')!;
const engine=new BrowserEngine();
const network=new NetworkGraph();
let snapshot:Snapshot|null=null;
let filename='';
let generation=0;
let pathGeneration=0;
let busy=false;
let error:Diagnostic|null=null;
let tab:Tab='overview';
let selectedBus:number|null=null;
let selectedEquipment:string|null=null;
let graphData:GraphData|null=null;
let graphTitle='Focused network';
let graphMessage='';
let path:Path|null=null;
let pathText='Choose two bus numbers to find a path.';
let pathError=false;
let pathBusy=false;
let fromBus='';
let toBus='';
let query='';
let page=0;
const PAGE_SIZE=25;
const fmt=(n:number)=>n.toLocaleString('en-US');
const esc=(s:unknown)=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
const button=(label:string,action:string,extra='')=>`<button class="button" data-action="${action}" ${extra}>${label}</button>`;

function render() {
  network.destroy();
  workspace.setAttribute('aria-busy',String(busy));
  document.querySelector('#view-title')!.textContent=tabs.find(t=>t[0]===tab)![1];
  for(const [id] of tabs) document.querySelector(`[data-tab="${id}"]`)!.classList.toggle('active',id===tab);
  for(const [id] of tabs) document.querySelector(`#nav-${id}`)!.textContent='';
  if (!snapshot) {
    workspace.innerHTML=`<div class="page-heading"><div><p class="eyebrow">POWER SYSTEM TOPOLOGY</p><h1>A clearer view of your network.</h1><p class="muted">From a RAW case to the connections that matter.</p></div><span class="tag">PSS/E RAW · v35</span></div>
      ${error?`<div class="error-banner" role="alert"><strong>${esc(error.section)}${error.line?` · line ${error.line}`:''}</strong><span>${esc(error.message)}</span></div>`:''}
      <section class="import-panel ${busy?'loading':''}" id="drop-zone" aria-label="RAW file drop zone">
        <div class="import-symbol">${logo}</div><p class="eyebrow">${busy?'READING YOUR NETWORK':'START WITH A CASE FILE'}</p><h2>${busy?'Building the topology…':'Open your power system network'}</h2>
        <p>${busy?esc(filename):'Drop a PSS/E RAW v35 file here, or choose one from your device.'}</p>
        ${busy?`<div class="progress-track"><span></span></div><p class="small muted">You can open another file to replace this analysis.</p>`:`<button class="button primary" data-action="choose">Choose RAW file</button><span class="import-note">UTF-8 and Windows-1252 supported</span>`}
      </section>
      <div class="feature-grid"><article><span class="feature-icon">◌</span><h3>Understand connectivity</h3><p>Find islands and see how buses connect across your network.</p></article><article><span class="feature-icon">◇</span><h3>Find critical connections</h3><p>Identify bridges and articulation buses in the active AC topology.</p></article><article><span class="feature-icon">⇢</span><h3>Trace a path</h3><p>Follow the shortest route by equipment hops, including transformers.</p></article></div>
      <div class="workspace-footer">YOUR CASE, YOUR DEVICE <span>Analysis runs locally. No account or upload required.</span></div>`;
    return;
  }
  const c=snapshot.case,a=snapshot.analysis;
  const lines=c.equipment.filter(e=>e.kind==='line').length;
  const transformers=c.equipment.filter(e=>e.kind==='transformer').length;
  const warnings=c.diagnostics.filter(d=>d.severity==='warning');
  const firstTitle=c.titles.find(t=>t.trim())?.trim();
  document.querySelector('#nav-islands')!.textContent=fmt(a.islands.length);
  document.querySelector('#nav-critical')!.textContent=fmt(a.bridges.length);
  document.querySelector('#nav-diagnostics')!.textContent=fmt(c.diagnostics.length);
  workspace.innerHTML=`<div class="page-heading"><div><p class="eyebrow">NETWORK ANALYSIS <span class="dot-separator">/</span> RAW v${c.revision}</p><h1 data-testid="filename">${esc(filename)}</h1><p class="muted case-meta">${c.base_mva} MVA base <span>·</span> ${c.frequency_hz} Hz <span>·</span> ${esc(c.encoding)} <span>·</span> In-service AC topology</p></div><div class="report-actions">${button('Export CSV','csv')}${button('Export JSON','json')}</div></div>
    ${firstTitle?`<details class="case-title"><summary>Case notes</summary>${c.titles.map(t=>`<p>${esc(t)}</p>`).join('')}</details>`:''}
    ${warnings.length?`<div class="warning-banner" role="status"><strong>Topology coverage is limited</strong><span>${warnings.map(d=>esc(d.message)+' '+esc(d.section)).join(' · ')}</span>${button('Review diagnostics','tab','data-tab="diagnostics"')}</div>`:''}
    <div class="metrics"><article><span>BUSES</span><strong data-testid="bus-count">${fmt(c.buses.length)}</strong><p>${fmt(a.active_buses)} active · ${fmt(a.excluded_buses)} excluded</p></article><article><span>TRANSMISSION LINES</span><strong data-testid="line-count">${fmt(lines)}</strong><p>Parallel circuits preserved</p></article><article><span>TRANSFORMERS</span><strong data-testid="transformer-count">${fmt(transformers)}</strong><p>${fmt(a.synthetic_junctions)} synthetic junctions</p></article><article class="metric-accent"><span>CONNECTED ISLANDS</span><strong>${fmt(a.islands.length)}</strong><p>${fmt(a.bridges.length)} bridges · ${fmt(a.articulation_buses.length)} cut buses</p></article></div>
    <div class="explorer-grid"><section class="panel network-panel"><div class="panel-heading"><div><p class="eyebrow">NETWORK VIEW</p><h2>${esc(graphTitle)}</h2></div>${button('Fit view','fit')}</div><div class="graph-wrap"><div id="network-canvas" aria-label="Focused network graph" role="img"></div>${!graphData?`<div class="graph-empty"><span>◌</span><h3>Select a smaller network view</h3><p>${esc(graphMessage||'Inspect a bus, open an island, or find a path to explore its connections.')}</p></div>`:''}</div><div class="graph-footer"><span data-testid="graph-status">${graphData?`${fmt(graphData.nodes.length)} nodes · ${fmt(graphData.edges.length)} connections`:'No graph selected'}</span><div class="legend"><span><i class="legend-bus"></i>Bus</span><span><i class="legend-cut"></i>Critical</span><span><i class="legend-path"></i>Path</span></div></div></section>
    <aside class="panel inspector" id="inspector">${inspectorHtml()}</aside></div>
    <section class="panel path-panel"><div><p class="eyebrow">SHORTEST PATH</p><h2>Trace a connection</h2></div><form id="path-form"><label>From bus<input name="from" aria-label="From bus" type="number" min="1" max="2147483647" step="1" required placeholder="Bus number" value="${esc(fromBus)}" /></label><span class="path-arrow" aria-hidden="true">→</span><label>To bus<input name="to" aria-label="To bus" type="number" min="1" max="2147483647" step="1" required placeholder="Bus number" value="${esc(toBus)}" /></label><button class="button primary" type="submit" ${pathBusy?'disabled':''}>${pathBusy?'Finding…':'Find path'}</button></form><div data-testid="path-result" class="path-result ${pathError?'text-error':''}" ${pathError?'role="alert"':''} aria-live="polite">${esc(pathText)}</div></section>
    <section class="panel results-panel"><div class="panel-heading"><div><p class="eyebrow">TOPOLOGY RESULTS</p><h2>${tabs.find(t=>t[0]===tab)![1]}</h2></div><label class="search"><span aria-hidden="true">⌕</span><input id="result-search" aria-label="${tab==='overview'||tab==='buses'?'Search buses':'Search results'}" placeholder="${tab==='overview'||tab==='buses'?'Search bus number or name…':'Search results…'}" value="${esc(query)}" /></label></div><div id="table-container"></div></section>
    <div class="workspace-footer">ACTIVE AC TOPOLOGY <span>${fmt(a.active_equipment)} equipment in graph · ${fmt(a.excluded_equipment)} excluded · Files remain in memory</span></div>`;
  renderTable();
  if (graphData) network.render(document.querySelector('#network-canvas')!,graphData,snapshot,path,(bus,eq)=>{
    selectedBus=bus;selectedEquipment=eq;
    document.querySelector('#inspector')!.innerHTML=inspectorHtml();
  });
  document.querySelector('#path-form')!.addEventListener('submit',findPath);
}

function sourceHtml(source:SourceRecord) {
  return `<details class="source-details"><summary>RAW fields · line ${source.line}</summary><dl class="field-list">${Object.entries(source.fields).map(([k,v])=>`<div><dt>${esc(k)}</dt><dd>${esc(v)||'—'}</dd></div>`).join('')}</dl></details>`;
}

function inspectorHtml():string {
  if (!snapshot) return '';
  const c=snapshot.case,a=snapshot.analysis;
  if(selectedEquipment) {
    const e=c.equipment.find(e=>e.id===selectedEquipment);
    if (!e) return '';
    const active=snapshot.graph.edges.some(g=>g.equipment_id===e.id);
    return `<p class="eyebrow">EQUIPMENT INSPECTOR</p><h2>${esc(e.kind)} · ${esc(e.circuit)}</h2><p class="muted wrap">${esc(e.name.trim()||e.id)}</p><span class="status-tag ${active?'':'inactive'}">${active?'In active graph':'Excluded from graph'}</span><dl class="detail-list"><div><dt>Terminals</dt><dd>${e.buses.map(b=>`<button class="text-button" data-action="inspect-bus" data-bus="${b}">${b}</button>`).join(' ↔ ')}</dd></div><div><dt>RAW status</dt><dd>${e.status}</dd></div><div><dt>Active windings</dt><dd>${e.active_terminals.map((v,i)=>v?i+1:null).filter(Boolean).join(', ')||'None'}</dd></div><div><dt>Source</dt><dd>${esc(e.source.section)} · ${e.source.line}</dd></div></dl>${sourceHtml(e.source)}`;
  }
  const bus=c.buses.find(b=>b.number===selectedBus);
  if(!bus) return `<p class="eyebrow">BUS INSPECTOR</p><div class="inspector-empty"><span>⊙</span><h3>Inspect a bus or connection</h3><p>Select a graph element or a table row to view its equipment and RAW fields.</p></div>`;
  const degree=a.degrees.find(d=>d.bus===bus.number);
  const connected=c.equipment.filter(e=>e.buses.includes(bus.number));
  const loads=c.loads.filter(i=>i.bus===bus.number),generators=c.generators.filter(i=>i.bus===bus.number);
  return `<p class="eyebrow">BUS INSPECTOR</p><h2>Bus ${bus.number}</h2><p class="bus-name">${esc(bus.name.trim())}</p><span class="status-tag ${bus.bus_type===4?'inactive':''}">${bus.bus_type===4?'Out of service':a.articulation_buses.includes(bus.number)?'Articulation bus':'In service'}</span><dl class="detail-list"><div><dt>Base voltage</dt><dd>${bus.base_kv} kV</dd></div><div><dt>Voltage</dt><dd>${bus.voltage_pu?.toFixed(4)??'—'} pu</dd></div><div><dt>Bus type</dt><dd>${({1:'Load (PQ)',2:'Generator (PV)',3:'Swing',4:'Disconnected'} as Record<number,string>)[bus.bus_type]}</dd></div><div><dt>Island / degree</dt><dd>${degree?`${degree.island} / ${degree.degree}`:'Excluded'}</dd></div><div><dt>Area / zone</dt><dd>${bus.area??'—'} / ${bus.zone??'—'}</dd></div></dl>${button('Show neighborhood','neighborhood',`data-bus="${bus.number}" ${bus.bus_type===4?'disabled':''}`)}
    <div class="inspector-section"><h3>Connected equipment <span>${connected.length}</span></h3><div class="equipment-list">${connected.map(e=>`<button data-action="inspect-equipment" data-equipment="${esc(e.id)}"><span class="equipment-kind">${e.kind==='transformer'?'⇄':e.kind==='switch'?'⋯':'─'}</span><span>${esc(e.kind)} ${esc(e.circuit)}<small>${e.buses.join(' ↔ ')}</small></span><span class="equipment-status">${snapshot!.graph.edges.some(g=>g.equipment_id===e.id)?'●':'○'}</span></button>`).join('')||'<p class="muted">No connected equipment.</p>'}</div></div>
    <div class="inspector-section"><h3>Loads (${loads.length}) / generators (${generators.length})</h3>${[...loads,...generators].map(i=>`<details><summary>${esc(i.id)} · ${i.status===1?'in service':'off'}</summary><p>${i.p_mw} MW · ${i.q_mvar} MVAr</p>${sourceHtml(i.source)}</details>`).join('')||'<p class="muted small">No load or generator records at this bus.</p>'}</div>${sourceHtml(bus.source)}`;
}

function tableRows():Record<string,unknown>[] {
  if(!snapshot) return [];
  const c=snapshot.case,a=snapshot.analysis;
  switch(tab) {
    case 'islands':return a.islands.map(i=>({island:i.id,buses:i.buses.length,junctions:i.synthetic_nodes.length,bus_numbers:i.buses}));
    case 'equipment':return c.equipment.map(e=>({id:e.id,kind:e.kind,circuit:e.circuit,buses:e.buses,name:e.name.trim(),status:e.status,active:snapshot!.graph.edges.some(g=>g.equipment_id===e.id),line:e.source.line}));
    case 'critical':return [...a.bridges.map(e=>({type:'Bridge',id:e.equipment_id,from:e.source,to:e.target,winding:e.winding??''})),...a.articulation_buses.map(b=>({type:'Articulation bus',id:String(b),from:`bus:${b}`,to:'',winding:''}))];
    case 'diagnostics':return c.diagnostics.map(d=>({severity:d.severity,section:d.section,line:d.line,message:d.message}));
    default:return c.buses.map(b=>{const d=a.degrees.find(d=>d.bus===b.number);return {bus:b.number,name:b.name.trim(),base_kv:b.base_kv,degree:d?.degree??'',island:d?.island??'',status:b.bus_type===4?'Excluded':'Active',articulation:a.articulation_buses.includes(b.number)};});
  }
}

function filteredRows() {const q=query.toLowerCase().trim();return tableRows().filter(row=>!q||Object.values(row).some(value=>String(value).toLowerCase().includes(q)));}

function renderTable() {
  const container=document.querySelector('#table-container');if(!container||!snapshot) return;
  const all=filteredRows();page=Math.min(page,Math.max(0,Math.ceil(all.length/PAGE_SIZE)-1));
  const rows=all.slice(page*PAGE_SIZE,(page+1)*PAGE_SIZE);
  const columns=Object.keys(tableRows()[0]??{});
  const labels:Record<string,string>={bus:'Bus',name:'Name',base_kv:'Base kV',degree:'Degree',island:'Island',status:'Status',articulation:'Cut bus',buses:'Buses',junctions:'Junctions',bus_numbers:'Bus numbers',id:'Equipment / bus',kind:'Type',circuit:'Circuit',active:'In graph',line:'RAW line',type:'Critical type',from:'From',to:'To',winding:'Winding',severity:'Level',section:'Section',message:'Diagnostic'};
  container.innerHTML=rows.length?`<div class="table-scroll"><table><thead><tr>${columns.map(k=>`<th scope="col">${labels[k]??esc(k)}</th>`).join('')}<th scope="col"><span class="sr-only">Action</span></th></tr></thead><tbody>${rows.map(row=>`<tr>${columns.map(k=>`<td ${k==='name'||k==='message'?'class="wide-cell"':''}>${Array.isArray(row[k])?esc((row[k] as number[]).slice(0,8).join(', '))+((row[k] as number[]).length>8?' …':''):typeof row[k]==='boolean'?(row[k]?'Yes':'—'):esc(row[k])||'—'}</td>`).join('')}<td>${tab==='overview'||tab==='buses'?`<button class="text-button" aria-label="Inspect bus ${row.bus}" data-action="inspect-bus" data-bus="${row.bus}">Inspect ↗</button>`:tab==='islands'?`<button class="text-button" data-action="island" data-island="${row.island}">View island ↗</button>`:tab==='equipment'||(tab==='critical'&&row.type==='Bridge')?`<button class="text-button" data-action="inspect-equipment" data-equipment="${esc(row.id)}">Inspect ↗</button>`:tab==='critical'?`<button class="text-button" data-action="inspect-bus" data-bus="${esc(row.id)}">Inspect ↗</button>`:''}</td></tr>`).join('')}</tbody></table></div><div class="pagination"><span>${fmt(page*PAGE_SIZE+1)}–${fmt(Math.min((page+1)*PAGE_SIZE,all.length))} of ${fmt(all.length)} results</span><div>${button('Previous','previous',page===0?'disabled':'')}<span>${page+1} / ${Math.ceil(all.length/PAGE_SIZE)}</span>${button('Next','next',(page+1)*PAGE_SIZE>=all.length?'disabled':'')}</div></div>`:`<div class="table-empty">${query?'No results match your search.':tab==='critical'?'No bridges or articulation buses found.':tab==='diagnostics'?'No parsing diagnostics.':'No results in this view.'}</div>`;
}

function selectGraph(data:GraphData,title:string) {
  graphTitle=title;
  if(data.nodes.length>500) {graphData=null;graphMessage=`This selection has ${fmt(data.nodes.length)} nodes. Choose a bus neighborhood or path to stay within the 500-node display limit.`;}
  else {graphData=data;graphMessage='';}
}

function clearPathSelection() {
  ++pathGeneration;path=null;pathBusy=false;pathError=false;
  pathText='Choose two bus numbers to find a path.';
}

async function openFile(file:File) {
  const ownGeneration=++generation;++pathGeneration;
  engine.dispose();filename=file.name;busy=true;error=null;snapshot=null;path=null;selectedBus=null;selectedEquipment=null;graphData=null;
  pathText='Choose two bus numbers to find a path.';pathError=false;pathBusy=false;fromBus='';toBus='';query='';page=0;tab='overview';render();
  try {
    const bytes=await file.arrayBuffer();
    if(ownGeneration!==generation) return;
    const result=await engine.load(bytes);
    if(ownGeneration!==generation) return;
    snapshot=result;busy=false;selectedBus=result.analysis.degrees[0]?.bus??null;
    if(selectedBus!==null) selectGraph(neighborhood(result.graph,selectedBus),`Bus ${selectedBus} · 2-hop neighborhood`);
    render();
  } catch(e) {
    if(ownGeneration!==generation) return;
    const detail=e as Partial<Diagnostic>;
    error={severity:'error',section:detail.section??'APPLICATION',line:detail.line??0,message:detail.message??String(e)};busy=false;render();
  }
}

async function findPath(event:Event) {
  event.preventDefault();if(!snapshot) return;
  const form=event.currentTarget as HTMLFormElement;
  fromBus=(form.elements.namedItem('from') as HTMLInputElement).value;toBus=(form.elements.namedItem('to') as HTMLInputElement).value;
  const ownGeneration=generation,ownPath=++pathGeneration;
  pathBusy=true;pathError=false;pathText='Finding the shortest equipment path…';render();
  try {
    const result=await engine.path(Number(fromBus),Number(toBus));
    if(ownGeneration!==generation||ownPath!==pathGeneration) return;
    path=result;pathBusy=false;
    if(!result) {pathText=`No active AC path connects bus ${fromBus} to bus ${toBus}.`;pathError=false;}
    else {pathText=`${result.hops} equipment hop${result.hops===1?'':'s'} · ${result.buses.join(' → ')}`;selectGraph(focusedGraph(snapshot!.graph,new Set(result.node_ids)),`Path · ${fromBus} → ${toBus}`);}
    render();
  } catch(e) {
    if(ownGeneration!==generation||ownPath!==pathGeneration) return;
    path=null;pathBusy=false;pathError=true;pathText=(e as Error).message??String(e);render();
  }
}

function download(data:string,type:string,extension:string) {
  const url=URL.createObjectURL(new Blob([data],{type}));
  const link=document.createElement('a');link.href=url;link.download=`${filename.replace(/\.raw$/i,'')}-topology.${extension}`;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
}

app.addEventListener('click',event=>{
  const element=(event.target as Element).closest<HTMLElement>('[data-action]');if(!element) return;
  const action=element.dataset.action;
  if(action==='choose') fileInput.click();
  else if(action==='tab') {tab=element.dataset.tab as Tab;query='';page=0;render();}
  else if(action==='fit') network.fit();
  else if(action==='inspect-bus') {selectedBus=Number(element.dataset.bus);selectedEquipment=null;document.querySelector('#inspector')!.innerHTML=inspectorHtml();}
  else if(action==='inspect-equipment') {selectedEquipment=element.dataset.equipment!;selectedBus=null;document.querySelector('#inspector')!.innerHTML=inspectorHtml();}
  else if(action==='neighborhood'&&snapshot) {clearPathSelection();selectGraph(neighborhood(snapshot.graph,Number(element.dataset.bus)),`Bus ${element.dataset.bus} · 2-hop neighborhood`);render();}
  else if(action==='island'&&snapshot) {const island=snapshot.analysis.islands.find(i=>i.id===Number(element.dataset.island))!;clearPathSelection();selectGraph(focusedGraph(snapshot.graph,new Set([...island.buses.map(b=>`bus:${b}`),...island.synthetic_nodes])),`Island ${island.id}`);render();}
  else if(action==='previous'||action==='next') {page+=action==='next'?1:-1;renderTable();}
  else if(action==='csv'&&snapshot) download(toCsv(filteredRows()),'text/csv;charset=utf-8','csv');
  else if(action==='json'&&snapshot) download(JSON.stringify({format_version:1,filename,case:snapshot.case,analysis:snapshot.analysis,path},null,2),'application/json','json');
});
app.addEventListener('input',event=>{if((event.target as HTMLElement).id==='result-search'){query=(event.target as HTMLInputElement).value;page=0;renderTable();}});
fileInput.addEventListener('change',()=>{const file=fileInput.files?.[0];if(file) void openFile(file);fileInput.value='';});
workspace.addEventListener('dragover',event=>{event.preventDefault();workspace.classList.add('dragging');});
workspace.addEventListener('dragleave',event=>{if(!workspace.contains(event.relatedTarget as Node)) workspace.classList.remove('dragging');});
workspace.addEventListener('drop',event=>{event.preventDefault();workspace.classList.remove('dragging');const file=event.dataTransfer?.files[0];if(file) void openFile(file);});
window.addEventListener('pagehide',()=>{engine.dispose();network.destroy();});
render();
