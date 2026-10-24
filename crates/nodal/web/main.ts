import './style.css';
import {initializeTheme} from './theme';
import {BrowserEngine} from './engine';
import {NetworkGraph} from './graph';
import {icon} from './icons';
import {saveExport} from './export';
import {initializeTitlebar} from './titlebar';
import {graphLegend} from './graph-presentation';
import type {GraphSelection} from './graph-presentation';
import {focusedGraph,neighborhood,nodeForBus,toCsv} from './view-model';
import {busRows as mergedBusRows,memberSearchText} from './topology-view';
import {FieldControls} from './field-controls';
import {filterGraph,matchesBus,matchesEquipment,fieldValues} from './field-view';
import {createColorPresentation,type ColorPresentation} from './field-colors';
import type {Snapshot,GraphData,Path,Diagnostic,SourceRecord} from './types';

const logo=`<svg viewBox="0 0 40 40" fill="none" aria-hidden="true"><path d="M10 11L29 10L21 29L10 11M10 11L7 30L21 29" stroke="currentColor" stroke-width="1.5"/><circle cx="10" cy="11" r="4" fill="currentColor"/><circle cx="29" cy="10" r="3" fill="currentColor"/><circle cx="21" cy="29" r="4" fill="currentColor"/><circle cx="7" cy="30" r="2" fill="currentColor"/></svg>`;
const tabs=[['overview','Overview','overview'],['islands','Islands','islands'],['buses','Buses','bus'],['critical','Critical connections','critical'],['equipment','Equipment','equipment'],['diagnostics','Diagnostics','diagnostics']] as const;
type Tab=typeof tabs[number][0];
const iconSvg=icon;
const app=document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML=`
  <aside class="rail">
    <a class="brand" href="/">${logo}<span>nodal<span class="brand-sub">NETWORK EXPLORER</span></span></a>
    <div class="rail-label">WORKSPACE</div>
    <nav aria-label="Analysis views">${tabs.map(([id,label,icon])=>`<button data-action="tab" data-tab="${id}" class="nav-item ${id==='overview'?'active':''}" ${id==='overview'?'aria-current="page"':''}><span class="nav-icon">${iconSvg(icon)}</span>${label}<span class="nav-count" id="nav-${id}"></span></button>`).join('')}</nav>
    <div class="rail-bottom"><span class="local-dot"></span> Local browser workspace<p>Files stay on your device.<br>Powered by Rust + WebAssembly.</p><span class="version">NODAL / 0.1</span></div>
  </aside>
  <main>
    <header class="topbar"><div class="breadcrumb">Workspace <span>/</span> <strong id="view-title">Overview</strong></div><div class="top-actions"><div class="theme-control" role="group" aria-label="Color theme"><button class="theme-button" type="button" data-theme-preference="light" aria-label="Light theme" title="Light theme" aria-pressed="false">${icon('sun')}</button><button class="theme-button" type="button" data-theme-preference="dark" aria-label="Dark theme" title="Dark theme" aria-pressed="false">${icon('moon')}</button><button class="theme-button" type="button" data-theme-preference="system" aria-label="System theme" title="System theme" aria-pressed="false">${icon('monitor')}</button></div><span class="privacy"><span class="local-dot"></span> LOCAL ONLY</span><label class="button primary file-button" for="raw-file">${icon('open')} Open case</label><input id="raw-file" type="file" accept=".raw,.RAW" aria-label="Open RAW file" class="file-input" /></div></header>
    <section id="workspace" aria-busy="false"></section>
  </main>`;
initializeTitlebar(app);

const workspace=document.querySelector<HTMLElement>('#workspace')!;
const fileInput=document.querySelector<HTMLInputElement>('#raw-file')!;
// Attach input listeners once
workspace.addEventListener('input',event=>{const input=event.target as HTMLInputElement;if(input.id==='neighborhood-hops'&&input.validity.valid)neighborhoodHops=input.valueAsNumber;});
workspace.addEventListener('change',event=>{const input=event.target as HTMLInputElement;if(input.type==='checkbox'&&input.getAttribute('aria-label')==='Treat zero-impedance branches as the same node')void changeTopology(input.checked);});
workspace.addEventListener('submit',event=>{const form=event.target as HTMLFormElement;if(form.id==='path-form')void findPath(event,form);});
fileInput.addEventListener('change',()=>{const file=fileInput.files?.[0];if(file) void openFile(file);fileInput.value='';});
const engine=new BrowserEngine();
const network=new NetworkGraph();
const fieldControls=new FieldControls();
let colorPresentation:ColorPresentation={nodeColors:{},edgeColors:{},legends:[]};
initializeTheme(document.querySelector<HTMLElement>('.theme-control')!);
document.addEventListener('themechange',()=>network.updateTheme());
let snapshot:Snapshot|null=null;
let filename='';
let generation=0;
let pathGeneration=0;
let busy=false;
let topologyBusy=false;
let topologyGeneration=0;
let error:Diagnostic|null=null;
let tab:Tab='overview';
let selectedBus:number|null=null;
let selectedEquipment:string|null=null;
let sourceRecordBus:number|null=null;
type FocusDescriptor={id?:string;action?:string;tab?:string;bus?:string;island?:string;equipment?:string;name?:string;type?:string;formSubmit?:boolean;ariaLabel?:string;owner?:string};
let pendingFocus:FocusDescriptor|null=null;
let graphData:GraphData|null=null;
let scopedGraph:GraphData|null=null;
let graphTitle='Focused network';
let graphMessage='';
let graphView:'full'|'island'|'neighborhood'|'path'='neighborhood';
let selectedIsland:number|null=null;
let selectedIslandAnchor:number|null=null;
let selectedNeighborhoodBus:number|null=null;
let appliedMerge=false;
let path:Path|null=null;
let pathText='Choose two bus numbers to find a path.';
let pathError=false;
let pathBusy=false;
let fromBus='';
let toBus='';
let neighborhoodHops=2;
let mergeZeroImpedance=false;
let query='';
let page=0;
const PAGE_SIZE=25;
const fmt=(n:number)=>n.toLocaleString('en-US');
const esc=(s:unknown)=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
const button=(label:string,action:string,extra='')=>`<button class="button" data-action="${action}" ${topologyBusy&&action!=='choose'?'disabled':''} ${extra}>${label}</button>`;

function fieldViewNote():string {
  if(!fieldControls.filters.length) return '';
  const count=fieldControls.filters.length;
  let note=`${count} field filter${count===1?'':'s'} applied within this view. Islands and paths use the full active topology.`;
  if(path&&graphView==='path') {
    const nodes=new Set(graphData?.nodes.map(node=>node.id));
    const edges=new Set(graphData?.edges.map(edge=>edge.id));
    if(path.node_ids.some(id=>!nodes.has(id))||path.connection_ids.some(id=>!edges.has(id))) note+=' Filters hide part of this path.';
  }
  return note;
}

function fieldColorLegendHtml():string {
  if(!snapshot||!graphData) return '';
  return colorPresentation.legends.map(legend=>{
    const scheme=fieldControls.colors[legend.target];
    const ids=legend.target==='bus'
      ?graphData!.nodes.filter(node=>node.bus!==null).map(node=>node.bus!)
      :[...new Set(graphData!.edges.map(edge=>edge.equipment_id))];
    if(!ids.length) return '';
    const visible=new Set<string>();
    let mixed=false,missing=false;
    for(const id of ids) {
      const values=fieldValues(snapshot!,legend.target,id,scheme.field);
      if(values.length>1) {mixed=true;continue;}
      if(!values.length||values[0]===null) missing=true;
      else visible.add(String(values[0]));
    }
    const entries=legend.entries.filter(entry=>visible.has(entry.label));
    if(missing) entries.push({label:'Missing',color:scheme.missing});
    if(mixed) entries.push({label:'Mixed',color:scheme.mixed});
    const swatches=entries.map(entry=>`<span data-color-value="${esc(entry.label)}"><i style="background-color:${entry.color}" aria-hidden="true"></i>${esc(entry.label)}</span>`).join('');
    const ramp=legend.gradient?`<span class="field-gradient"><span>${esc(legend.gradient.min)}</span><i style="background:linear-gradient(to right,${legend.gradient.low},${legend.gradient.high})" aria-hidden="true"></i><span>${esc(legend.gradient.max)}</span></span>`:'';
    return `<div class="field-legend-group"><strong>${legend.target==='bus'?'Buses':'Equipment'} · ${esc(legend.label)}</strong><div class="field-legend-values">${ramp}${swatches}</div></div>`;
  }).join('');
}

function rememberFocus():void {
  const active=document.activeElement;
  if(!(active instanceof HTMLElement)||active===document.body||!app.contains(active)) return;
  const descriptor:FocusDescriptor={};
  if(active.id) descriptor.id=active.id;
  if(active.dataset.action) descriptor.action=active.dataset.action;
  for(const key of ['tab','bus','island','equipment'] as const) if(active.dataset[key]) descriptor[key]=active.dataset[key];
  if(active.closest('#path-form')&&active.tagName==='BUTTON') descriptor.formSubmit=true;
  if(active.getAttribute('name')) descriptor.name=active.getAttribute('name')!;
  if(active.getAttribute('type')) descriptor.type=active.getAttribute('type')!;
  if(active.getAttribute('aria-label')) descriptor.ariaLabel=active.getAttribute('aria-label')!;
  const owner=active.closest<HTMLElement>('.report-actions, .inspector-topology, .import-panel');
  if(owner) descriptor.owner=owner.className;
  if(Object.keys(descriptor).length) pendingFocus=descriptor;
}

function restoreFocus():void {
  if(!pendingFocus) return;
  let target:HTMLElement|null=null;
  const d=pendingFocus;
  if(d.id) target=document.getElementById(d.id);
  else if(d.action) target=Array.from(app.querySelectorAll<HTMLElement>('[data-action]')).find(el=>el.dataset.action===d.action&&(!d.tab||el.dataset.tab===d.tab)&&(!d.bus||el.dataset.bus===d.bus)&&(!d.island||el.dataset.island===d.island)&&(!d.equipment||el.dataset.equipment===d.equipment))??null;
  else if(d.formSubmit) target=document.querySelector<HTMLElement>('#path-form button[type="submit"]');
  else if(d.name) target=Array.from(app.querySelectorAll<HTMLElement>('[name]')).find(el=>el.getAttribute('name')===d.name)??null;
  else if(d.ariaLabel) {
    const root=d.owner?Array.from(app.querySelectorAll<HTMLElement>('.report-actions, .inspector-topology, .import-panel')).find(el=>el.className===d.owner)??app:app;
    target=Array.from(root.querySelectorAll<HTMLElement>('[aria-label]')).find(el=>el.getAttribute('aria-label')===d.ariaLabel)??null;
  }
  if(!target||('disabled' in target&&(target as HTMLButtonElement|HTMLInputElement).disabled)) return;
  target.focus();
  pendingFocus=null;
}

function render() {
  rememberFocus();
  network.destroy();
  workspace.setAttribute('aria-busy',String(busy||topologyBusy));
  document.querySelector('#view-title')!.textContent=tabs.find(t=>t[0]===tab)![1];
  for(const [id] of tabs) {const navItem=document.querySelector<HTMLButtonElement>(`[data-tab="${id}"]`)!;navItem.disabled=!snapshot;navItem.classList.toggle('active',id===tab);if(id===tab)navItem.setAttribute('aria-current','page');else navItem.removeAttribute('aria-current');}
  for(const [id] of tabs) document.querySelector(`#nav-${id}`)!.textContent='';
  if (!snapshot) {
    workspace.innerHTML=`<div class="page-heading"><div><p class="eyebrow">POWER SYSTEM TOPOLOGY</p><h1>A clearer view of your network.</h1><p class="muted">From a RAW case to the connections that matter.</p></div><span class="tag">PSS/E RAW · v35</span></div>
      ${error?`<div class="error-banner" role="alert"><strong>${esc(error.section)}${error.line?` · line ${error.line}`:''}</strong><span>${esc(error.message)}</span></div>`:''}
      <section class="import-panel ${busy?'loading':''}" id="drop-zone" aria-label="RAW file drop zone">
        <div class="import-symbol">${logo}</div><p class="eyebrow">${busy?'READING YOUR NETWORK':'START WITH A CASE FILE'}</p><h2>${busy?'Building the topology…':'Open your power system network'}</h2>
        <p>${busy?esc(filename):'Drop a PSS/E RAW v35 file here, or choose one from your device.'}</p>
        <label class="global-topology-option"><input type="checkbox" aria-label="Treat zero-impedance branches as the same node" ${mergeZeroImpedance?'checked':''} ${busy?'disabled':''} />Treat zero-impedance branches as the same node</label>
        ${busy?`<div class="progress-track"><span></span></div><p class="small muted">You can open another file to replace this analysis.</p>`:`<button class="button primary" data-action="choose">Choose RAW file</button><span class="import-note">UTF-8 and Windows-1252 supported</span>`}
      </section>
      <div class="feature-grid"><article><span class="feature-icon">${icon('islands')}</span><h3>Understand connectivity</h3><p>Find islands and see how buses connect across your network.</p></article><article><span class="feature-icon">${icon('critical')}</span><h3>Find critical connections</h3><p>Identify bridges and articulation buses in the active AC topology.</p></article><article><span class="feature-icon">${icon('trace')}</span><h3>Trace a path</h3><p>Follow the shortest route by equipment hops, including transformers.</p></article></div>
      <div class="workspace-footer">YOUR CASE, YOUR DEVICE <span>Analysis runs locally. No account or upload required.</span></div>`;
    restoreFocus();
    return;
  }
  const c=snapshot.case,a=snapshot.analysis;
  colorPresentation=createColorPresentation(snapshot,fieldControls.colors);
  appliedMerge=snapshot.graph.merge_zero_impedance;
  const lines=c.equipment.filter(e=>e.kind==='line').length;
  const transformers=c.equipment.filter(e=>e.kind==='transformer').length;
  const warnings=c.diagnostics.filter(d=>d.severity==='warning');
  const firstTitle=c.titles.find(t=>t.trim())?.trim();
  document.querySelector('#nav-islands')!.textContent=fmt(a.islands.length);
  document.querySelector('#nav-critical')!.textContent=fmt(a.bridges.length);
  document.querySelector('#nav-diagnostics')!.textContent=fmt(c.diagnostics.length);
  workspace.innerHTML=`<div class="page-heading"><div><p class="eyebrow">NETWORK ANALYSIS <span class="dot-separator">/</span> RAW v${c.revision}</p><h1 data-testid="filename">${esc(filename)}</h1><p class="muted case-meta">${c.base_mva} MVA base <span>·</span> ${c.frequency_hz} Hz <span>·</span> ${esc(c.encoding)} <span>·</span> In-service AC topology</p></div><div class="report-actions"><label class="global-topology-option"><input type="checkbox" aria-label="Treat zero-impedance branches as the same node" ${appliedMerge?'checked':''} ${topologyBusy?'disabled':''} />Treat zero-impedance branches as the same node</label>${button('Export CSV','csv',topologyBusy?'disabled':'')}${button('Export JSON','json',topologyBusy?'disabled':'')}</div></div>
    ${error?`<div class="error-banner" role="alert"><strong>${esc(error.section)}</strong><span>${esc(error.message)}</span></div>`:''}
    ${firstTitle?`<details class="case-title"><summary>Case notes</summary>${c.titles.map(t=>`<p>${esc(t)}</p>`).join('')}</details>`:''}
    ${warnings.length?`<div class="warning-banner" role="status"><strong>Topology coverage is limited</strong><span>${warnings.map(d=>esc(d.message)+' '+esc(d.section)).join(' · ')}</span>${button('Review diagnostics','tab','data-tab="diagnostics"')}</div>`:''}
    <div class="metrics"><article><span>RAW BUSES</span><strong data-testid="bus-count">${fmt(c.buses.length)}</strong><p>${fmt(a.original_active_buses)} active source · ${fmt(a.excluded_buses)} excluded</p></article><article><span>TRANSMISSION LINES</span><strong data-testid="line-count">${fmt(lines)}</strong><p>Parallel circuits preserved</p></article><article><span>TRANSFORMERS</span><strong data-testid="transformer-count">${fmt(transformers)}</strong><p>${fmt(a.synthetic_junctions)} synthetic junctions</p></article><article class="metric-accent"><span>ANALYSIS TOPOLOGY</span><strong>${fmt(a.active_buses)}</strong><p>${fmt(a.islands.length)} islands · ${fmt(a.bridges.length)} bridges · ${fmt(a.articulation_buses.length)} cut buses</p></article></div>
    <div class="analysis-workspace"><section class="panel network-panel"><div class="panel-heading"><div><p class="eyebrow">NETWORK VIEW</p><h2>${esc(graphTitle)}</h2></div><div>${button('Show full network','full',`${topologyBusy?'disabled':''}`)} ${button('Fit view','fit',topologyBusy?'disabled':'')}</div></div><div id="field-controls-host" ${topologyBusy?'inert':''}></div><p class="field-view-note" data-testid="field-view-note" aria-live="polite">${esc(fieldViewNote())}</p><div class="graph-wrap"><div id="network-canvas" aria-label="Focused network graph" role="img"></div>${topologyBusy?`<div class="graph-empty"><span>◌</span><h3>Rebuilding topology…</h3><p>Graph, table, path, and inspection actions are paused until the new topology is ready.</p></div>`:!graphData?`<div class="graph-empty"><span>◌</span><h3>Select a smaller network view</h3><p>${esc(graphMessage||'Inspect a bus, open an island, or find a path to explore its connections.')}</p></div>`:graphData.nodes.length===0?'<div class="graph-empty"><span>◌</span><h3>No elements match these filters</h3><p>Adjust or clear the field filters to restore this view.</p></div>':''}</div><div class="graph-footer"><span data-testid="graph-status">${topologyBusy?'Rebuilding topology':graphData?`${fmt(graphData.nodes.length)} nodes · ${fmt(graphData.edges.length)} connections`:'No graph selected'}</span><div class="legend">${snapshot&&graphData?graphLegend(graphData,snapshot,path,sourceRecordBus===null?{bus:selectedBus,equipment:selectedEquipment}:{bus:null,equipment:null}).map(entry=>`<span class="legend-${entry.key}"><i aria-hidden="true"></i>${entry.label}</span>`).join(''):''}</div></div><div class="field-color-legend">${fieldColorLegendHtml()}</div><p class="topology-explanation">Topology bridges indicate connectivity, not outage consequences. Three-winding transformer junctions are synthetic; a transformer counts as one equipment hop.</p></section>
    <aside class="panel inspector" id="inspector"><div class="inspector-content">${inspectorHtml()}</div></aside>
    <section class="panel results-panel"><div class="panel-heading"><div><p class="eyebrow">TOPOLOGY RESULTS</p><h2>${tabs.find(t=>t[0]===tab)![1]}</h2></div><label class="search">${icon('search')}<input id="result-search" aria-label="${tab==='overview'||tab==='buses'?'Search buses':'Search results'}" placeholder="${tab==='overview'||tab==='buses'?'Search bus number or name…':'Search results…'}" value="${esc(query)}" /></label></div><div id="table-container"></div></section></div>
    <section class="panel path-panel"><div><p class="eyebrow">SHORTEST PATH</p><h2>Trace a connection</h2></div><form id="path-form"><label>From bus<input name="from" aria-label="From bus" type="number" min="1" max="2147483647" step="1" required placeholder="Bus number" value="${esc(fromBus)}" ${topologyBusy?'disabled':''} /></label><span class="path-arrow" aria-hidden="true">→</span><label>To bus<input name="to" aria-label="To bus" type="number" min="1" max="2147483647" step="1" required placeholder="Bus number" value="${esc(toBus)}" ${topologyBusy?'disabled':''} /></label><button class="button primary" type="submit" ${pathBusy||topologyBusy?'disabled':''}>${pathBusy?'Finding…':'Find path'}</button></form><div data-testid="path-result" class="path-result ${pathError?'text-error':''}" ${pathError?'role="alert"':''} aria-live="polite">${esc(pathText)}</div></section>

    <div class="workspace-footer">ACTIVE AC TOPOLOGY <span>${fmt(a.active_equipment)} equipment in graph · ${fmt(a.absorbed_equipment)} absorbed · ${fmt(a.excluded_equipment)} excluded · Files remain in memory</span></div>`;
  renderTable();
  fieldControls.mount(document.querySelector<HTMLElement>('#field-controls-host')!,snapshot,kind=>{
    if(!snapshot||topologyBusy) return;
    if(kind==='filters') {page=0;refreshGraph();render();}
    else {
      colorPresentation=createColorPresentation(snapshot,fieldControls.colors);
      network.setColors(colorPresentation);
      document.querySelector<HTMLElement>('.field-color-legend')!.innerHTML=fieldColorLegendHtml();
    }
  });
  if (graphData&&!topologyBusy) {
    network.render(document.querySelector<HTMLElement>('#network-canvas')!,graphData,snapshot,path,(bus,eq)=>selectInspection({bus,equipment:eq}),colorPresentation);
    network.setSelection(sourceRecordBus===null?{bus:selectedBus,equipment:selectedEquipment}:{bus:null,equipment:null});
  }
  if (snapshot) {
  const pathForm=document.querySelector('#path-form')!;
  if(topologyBusy) (pathForm.querySelector('button') as HTMLButtonElement).disabled=true;
  const pathResultEl=document.querySelector('[data-testid="path-result"]');
  if(pathResultEl) {
    pathResultEl.textContent=pathText;
  } else {
    const p=document.createElement('p');
    p.dataset.testid='path-result';
    pathForm.appendChild(p);
  }
}
  restoreFocus();
}

function selectInspection(selection:GraphSelection):void {
  selectedBus=selection.bus;
  selectedEquipment=selection.equipment;
  sourceRecordBus=null;
  if(snapshot) {
    const active=document.activeElement;
    const restoreInspectorFocus=active instanceof HTMLElement&&!!active.closest('#inspector');
    const inspector=document.querySelector<HTMLElement>('#inspector .inspector-content');
    if(inspector) {inspector.innerHTML=inspectorHtml();if(restoreInspectorFocus)inspector.querySelector<HTMLElement>('h2')?.focus();}
    renderSelection();
    network.setSelection(selection);
    const legend=document.querySelector<HTMLElement>('.legend');
    if(legend&&graphData) legend.innerHTML=graphLegend(graphData,snapshot,path,selection).map(entry=>`<span class="legend-${entry.key}"><i aria-hidden="true"></i>${entry.label}</span>`).join('');
  }
}

function renderSelection():void {
  for(const row of document.querySelectorAll<HTMLTableRowElement>('#table-container tbody tr')) {
    const action=row.querySelector<HTMLElement>('[data-action="inspect-bus"]');
    const equipmentAction=row.querySelector<HTMLElement>('[data-action="inspect-equipment"]');
    const isSelected=!!action&&action.dataset.bus===String(selectedBus)&&sourceRecordBus===null||!!equipmentAction&&equipmentAction.dataset.equipment===selectedEquipment;
    row.dataset.selected=String(isSelected);
    const firstCell=row.querySelector<HTMLTableCellElement>('td');
    if(firstCell) {firstCell.querySelector('.selected-label')?.remove();if(isSelected)firstCell.insertAdjacentHTML('afterbegin','<span class="selected-label">Selected</span>');}
    const button=row.querySelector<HTMLButtonElement>('[data-action="inspect-bus"]');
    if(button&&action) button.setAttribute('aria-label',`Inspect bus ${action.dataset.bus}`);
  }
}

function sourceHtml(source:SourceRecord) {
  return `<details class="source-details"><summary>RAW fields · line ${source.line}</summary><dl class="field-list">${Object.entries(source.fields).map(([k,v])=>`<div><dt>${esc(k)}</dt><dd>${esc(v)||'—'}</dd></div>`).join('')}</dl></details>`;
}

function inspectorHtml():string {
  if (!snapshot) return '';
  const c=snapshot.case,a=snapshot.analysis;
  if(sourceRecordBus!==null) {
    const source=c.buses.find(b=>b.number===sourceRecordBus);
    if(source) return `<p class="eyebrow">SOURCE BUS RECORD</p><h2 tabindex="-1">Bus ${source.number}</h2><p class="bus-name">${esc(source.name.trim())}</p>${sourceHtml(source.source)}${button('Back to merged node','inspect-bus',`data-bus="${nodeForBus(snapshot.graph,source.number)?.bus??source.number}"`)}`;
  }
  if(selectedEquipment) {
    const e=c.equipment.find(e=>e.id===selectedEquipment);
    if (!e) return '';
    const active=snapshot.graph.edges.some(g=>g.equipment_id===e.id);
    const absorbed=snapshot.graph.absorbed_equipment_ids.includes(e.id);
    return `<p class="eyebrow">EQUIPMENT INSPECTOR</p><h2 tabindex="-1">${esc(e.kind)} · ${esc(e.circuit)}</h2><p class="muted wrap">${esc(e.name.trim()||e.id)}</p><span class="status-tag ${active?'':absorbed?'absorbed':'inactive'}">${active?'In active graph':absorbed?'Absorbed inside merged node':'Excluded from graph'}</span><dl class="detail-list"><div><dt>Terminals</dt><dd>${e.buses.map(b=>`<button class="text-button" data-action="inspect-bus" data-bus="${b}" ${topologyBusy?'disabled':''}>${b}</button>`).join(' ↔ ')}</dd></div><div><dt>RAW status</dt><dd>${e.status}</dd></div><div><dt>Active windings</dt><dd>${e.active_terminals.map((v,i)=>v?i+1:null).filter(Boolean).join(', ')||'None'}</dd></div><div><dt>Source</dt><dd>${esc(e.source.section)} · ${e.source.line}</dd></div></dl>${sourceHtml(e.source)}`;
  }
  const node=selectedBus===null?undefined:nodeForBus(snapshot.graph,selectedBus);
  const representative=node?.bus??selectedBus;
  const members=node?.member_buses?.length?node.member_buses:(representative===null||representative===undefined?[]:[representative]);
  const bus=c.buses.find(b=>b.number===representative);
  if(!bus) return `<p class="eyebrow">BUS INSPECTOR</p><div class="inspector-empty"><span>⊙</span><h3>Inspect a bus or connection</h3><p>Select a graph element or a table row to view its equipment and RAW fields.</p></div>`;
  const degree=a.degrees.find(d=>d.bus===bus.number);
  const memberSet=new Set(members);
  const connected=c.equipment.filter(e=>e.buses.some(n=>memberSet.has(n)));
  const loads=c.loads.filter(i=>memberSet.has(i.bus)),generators=c.generators.filter(i=>memberSet.has(i.bus));
  const active=!!node;
  const threshold=c.zero_impedance_threshold;
  return `<p class="eyebrow">BUS INSPECTOR</p><h2 tabindex="-1">${members.length>1?`Merged node ${bus.number} · ${members.length} buses`:`Bus ${bus.number}`}</h2><p class="bus-name">${esc(bus.name.trim())}</p><span class="status-tag ${bus.bus_type===4?'inactive':''}">${bus.bus_type===4?'Out of service':a.articulation_buses.includes(bus.number)?'Articulation bus':'In service'}</span><label class="global-topology-option inspector-topology"><input type="checkbox" aria-label="Treat zero-impedance branches as the same node" ${appliedMerge?'checked':''} ${topologyBusy?'disabled':''} />Treat zero-impedance branches as the same node</label>${members.length>1?`<div class="inspector-section"><h3>Member buses <span>${members.length}</span></h3><div class="equipment-list">${members.map(number=>{const member=c.buses.find(b=>b.number===number)!;return `<button data-action="source-bus" data-bus="${number}" ${topologyBusy?'disabled':''}><span>${number}<small>${esc(member.name.trim())}</small></span><span class="equipment-status">RAW ↗</span></button>`;}).join('')}</div></div>`:''}<dl class="detail-list"><div><dt>Base voltage</dt><dd>${bus.base_kv} kV</dd></div><div><dt>Voltage</dt><dd>${bus.voltage_pu?.toFixed(4)??'—'} pu</dd></div><div><dt>Bus type</dt><dd>${({1:'Load (PQ)',2:'Generator (PV)',3:'Swing',4:'Disconnected'} as Record<number,string>)[bus.bus_type]}</dd></div><div><dt>Island / degree</dt><dd>${degree?`${degree.island} / ${degree.degree}`:'Excluded'}</dd></div><div><dt>Area / zone</dt><dd>${bus.area??'—'} / ${bus.zone??'—'}</dd></div></dl><div class="neighborhood-controls"><label for="neighborhood-hops">Equipment hops</label><input id="neighborhood-hops" aria-label="Equipment hops" type="number" min="0" max="2147483647" step="1" required value="${neighborhoodHops}" ${active&&!topologyBusy?'':'disabled'} />${appliedMerge&&threshold===0?'<small class="threshold-note">THRSHZ is 0, so no zero-impedance branches are grouped.</small>':threshold>0?`<small class="threshold-note">THRSHZ ${threshold} p.u. · ${a.absorbed_equipment} absorbed equipment</small>`:''}${button('Show neighborhood','neighborhood',`data-bus="${bus.number}" ${active&&!topologyBusy?'':'disabled'}`)}${button('Export neighborhood CSV','neighborhood-csv',`data-bus="${bus.number}" ${active&&!topologyBusy?'':'disabled'}`)}</div>
    <div class="inspector-section"><h3>Connected equipment <span>${connected.length}</span></h3><div class="equipment-list">${connected.map(e=>`<button data-action="inspect-equipment" data-equipment="${esc(e.id)}" ${topologyBusy?'disabled':''}><span class="equipment-kind">${icon('equipment')}</span><span>${esc(e.kind)} ${esc(e.circuit)}<small>${e.buses.join(' ↔ ')}</small></span><span class="equipment-status">${snapshot!.graph.edges.some(g=>g.equipment_id===e.id)?'In graph':snapshot!.graph.absorbed_equipment_ids.includes(e.id)?'Absorbed':'Excluded'}</span></button>`).join('')||'<p class="muted">No connected equipment.</p>'}</div></div>
    <div class="inspector-section"><h3>Loads (${loads.length}) / generators (${generators.length})</h3>${[...loads,...generators].map(i=>`<details><summary>${esc(i.id)} · bus ${i.bus} · ${i.status===1?'in service':'off'}</summary><p>${i.p_mw} MW · ${i.q_mvar} MVAr</p>${sourceHtml(i.source)}</details>`).join('')||'<p class="muted small">No load or generator records in this node.</p>'}</div>${sourceHtml(bus.source)}`;
}

function tableRows():Record<string,unknown>[] {
  if(!snapshot) return [];
  const c=snapshot.case,a=snapshot.analysis;
  switch(tab) {
    case 'islands':return a.islands.map(i=>({island:i.id,buses:i.buses.length,junctions:i.synthetic_nodes.length,bus_numbers:i.buses}));
    case 'equipment':return c.equipment.map(e=>({id:e.id,kind:e.kind,circuit:e.circuit,buses:e.buses,name:e.name.trim(),status:e.status,topology:snapshot!.graph.edges.some(g=>g.equipment_id===e.id)?'Active':snapshot!.graph.absorbed_equipment_ids.includes(e.id)?'Absorbed':'Excluded',line:e.source.line}));
    case 'critical':return [...a.bridges.map(e=>({type:'Bridge',id:e.equipment_id,from:e.source,to:e.target,winding:e.winding??''})),...a.articulation_buses.map(b=>({type:'Articulation bus',id:String(b),from:`bus:${b}`,to:'',winding:''}))];
    case 'diagnostics':return c.diagnostics.map(d=>({severity:d.severity,section:d.section,line:d.line,message:d.message}));
    default:return busRows();
  }
}

function busRows():Record<string,unknown>[] {return snapshot?mergedBusRows(snapshot):[];}

function filteredRows() {
  const q=query.toLowerCase().trim();
  return tableRows().filter(row=>{
    if(snapshot&&fieldControls.filters.length) {
      if((tab==='overview'||tab==='buses')&&!matchesBus(snapshot,Number(row.bus),fieldControls.filters)) return false;
      if(tab==='equipment'&&!matchesEquipment(snapshot,String(row.id),fieldControls.filters)) return false;
    }
    return !q||Object.values(row).some(value=>String(value).toLowerCase().includes(q))||(!!snapshot&&typeof row.bus==='number'&&(()=>{const node=nodeForBus(snapshot!.graph,row.bus as number);return !!node&&memberSearchText(snapshot!.graph,snapshot!.case.buses,node).includes(q);})());
  });
}

function renderTable() {
  const container=document.querySelector('#table-container');if(!container||!snapshot) return;
  const all=filteredRows();page=Math.min(page,Math.max(0,Math.ceil(all.length/PAGE_SIZE)-1));
  const rows=all.slice(page*PAGE_SIZE,(page+1)*PAGE_SIZE);
  const columns=Object.keys(tableRows()[0]??{});
  const labels:Record<string,string>={bus:'Bus',name:'Name',member_buses:'Member buses',base_kv:'Base kV',degree:'Degree',island:'Island',status:'Status',articulation:'Cut bus',buses:'Buses',junctions:'Junctions',bus_numbers:'Bus numbers',id:'Equipment / bus',kind:'Type',circuit:'Circuit',active:'In graph',topology:'Topology status',line:'RAW line',type:'Critical type',from:'From',to:'To',winding:'Winding',severity:'Level',section:'Section',message:'Diagnostic'};
  container.innerHTML=rows.length?`<div class="table-scroll"><table><thead><tr>${columns.map(k=>`<th scope="col">${labels[k]??esc(k)}</th>`).join('')}<th scope="col"><span class="sr-only">Action</span></th></tr></thead><tbody>${rows.map(row=>`<tr>${columns.map(k=>`<td ${k==='name'||k==='message'?'class="wide-cell"':''}>${Array.isArray(row[k])?esc((row[k] as number[]).slice(0,8).join(', '))+((row[k] as number[]).length>8?' …':''):typeof row[k]==='boolean'?(row[k]?'Yes':'—'):esc(row[k])||'—'}</td>`).join('')}<td>${tab==='overview'||tab==='buses'?`<button class="text-button" aria-label="Inspect bus ${row.bus}" data-action="inspect-bus" data-bus="${row.bus}" ${topologyBusy?'disabled':''}>Inspect ↗</button>`:tab==='islands'?`<button class="text-button" data-action="island" data-island="${row.island}" ${topologyBusy?'disabled':''}>View island ↗</button>`:tab==='equipment'||(tab==='critical'&&row.type==='Bridge')?`<button class="text-button" data-action="inspect-equipment" data-equipment="${esc(row.id)}" ${topologyBusy?'disabled':''}>Inspect ↗</button>`:tab==='critical'?`<button class="text-button" data-action="inspect-bus" data-bus="${esc(row.id)}" ${topologyBusy?'disabled':''}>Inspect ↗</button>`:''}</td></tr>`).join('')}</tbody></table></div><div class="pagination"><span>${fmt(page*PAGE_SIZE+1)}–${fmt(Math.min((page+1)*PAGE_SIZE,all.length))} of ${fmt(all.length)} results</span><div>${button('Previous','previous',page===0?'disabled':'')}<span>${page+1} / ${Math.ceil(all.length/PAGE_SIZE)}</span>${button('Next','next',(page+1)*PAGE_SIZE>=all.length?'disabled':'')}</div></div>`:`<div class="table-empty">${fieldControls.filters.length?'No results match your search and field filters.':query?'No results match your search.':tab==='critical'?'No bridges or articulation buses found.':tab==='diagnostics'?'No parsing diagnostics.':'No results in this view.'}</div>`;
  if(fieldControls.filters.length&&tab!=='overview'&&tab!=='buses'&&tab!=='equipment') container.insertAdjacentHTML('afterbegin','<p class="field-table-note">Field filters apply to bus and equipment rows. This table shows whole-case topology results and diagnostics.</p>');
  renderSelection();
}

function selectGraph(data:GraphData,title:string) {
  graphTitle=title;
  scopedGraph=data;
  refreshGraph();
}

function refreshGraph():void {
  const data=snapshot&&scopedGraph?filterGraph(snapshot,scopedGraph,fieldControls.filters):null;
  if(data&&data.nodes.length>500) {graphData=null;graphMessage=`This selection has ${fmt(data.nodes.length)} nodes. Choose a bus neighborhood or path to stay within the 500-node display limit. Field filters can also reduce the displayed selection.`;}
  else {graphData=data;graphMessage='';}
}

function setView(view:'full'|'island'|'neighborhood'|'path') {graphView=view;}

function restoreView(next:Snapshot) {
  if(graphView==='full') selectGraph(next.graph,'Full network');
  else if(graphView==='island'&&selectedIslandAnchor!==null) {
    const anchor=nodeForBus(next.graph,selectedIslandAnchor)?.bus??selectedIslandAnchor;
    const island=next.analysis.islands.find(i=>i.buses.includes(anchor));
    if(island) {selectedIsland=island.id;selectGraph(focusedGraph(next.graph,new Set([...island.buses.map(b=>nodeForBus(next.graph,b)?.id??`bus:${b}`),...island.synthetic_nodes])),`Island ${island.id}`);}
    else {graphView='neighborhood';restoreView(next);}
  } else {
    graphView='neighborhood';
    const origin=selectedNeighborhoodBus??next.analysis.degrees[0]?.bus??null;
    selectedNeighborhoodBus=origin;
    if(origin!==null) selectGraph(neighborhood(next.graph,origin,neighborhoodHops),`Bus ${origin} · ${neighborhoodHops}-hop neighborhood`);
    else {scopedGraph=null;graphData=null;graphTitle='Focused network';}
  }
}

function clearPathSelection() {
  ++pathGeneration;path=null;pathBusy=false;pathError=false;
  pathText='Choose two bus numbers to find a path.';
}

async function openFile(file:File) {
  const ownGeneration=++generation;++pathGeneration;
  fieldControls.resetFilters();
  ++topologyGeneration;engine.dispose();filename=file.name;busy=true;topologyBusy=false;error=null;snapshot=null;path=null;selectedBus=null;selectedEquipment=null;sourceRecordBus=null;scopedGraph=null;graphData=null;
  pathText='Choose two bus numbers to find a path.';pathError=false;pathBusy=false;fromBus='';toBus='';neighborhoodHops=2;query='';page=0;tab='overview';graphView='neighborhood';selectedIsland=null;selectedIslandAnchor=null;selectedNeighborhoodBus=null;render();
  try {
    const bytes=await file.arrayBuffer();
    if(ownGeneration!==generation) return;
    const result=await engine.load(bytes,mergeZeroImpedance);
    if(ownGeneration!==generation) return;
    snapshot=result;busy=false;mergeZeroImpedance=result.graph.merge_zero_impedance;selectedBus=result.analysis.degrees[0]?.bus??null;selectedNeighborhoodBus=selectedBus;
    if(selectedBus!==null) selectGraph(neighborhood(result.graph,selectedBus),`Bus ${selectedBus} · 2-hop neighborhood`);
    render();
  } catch(e) {
    if(ownGeneration!==generation) return;
    const detail=e as Partial<Diagnostic>;
    error={severity:'error',section:detail.section??'APPLICATION',line:detail.line??0,message:detail.message??String(e)};busy=false;render();
  }
}

async function changeTopology(enabled:boolean) {
  if(!snapshot||topologyBusy||enabled===snapshot.graph.merge_zero_impedance) {mergeZeroImpedance=snapshot?.graph.merge_zero_impedance??enabled;render();return;}
  const ownGeneration=generation,ownTopology=++topologyGeneration,prior=snapshot.graph.merge_zero_impedance;
  mergeZeroImpedance=enabled;topologyBusy=true;error=null;++pathGeneration;path=null;pathBusy=false;pathError=false;pathText='Choose two bus numbers to find a path.';render();
  try {
    const next=await engine.setMergeZeroImpedance(enabled);
    if(ownGeneration!==generation||ownTopology!==topologyGeneration) return;
    snapshot=next;mergeZeroImpedance=next.graph.merge_zero_impedance;topologyBusy=false;selectedEquipment=null;
    if(selectedBus!==null) selectedBus=nodeForBus(next.graph,selectedBus)?.bus??selectedBus;
    restoreView(next);render();
  } catch(e) {
    if(ownGeneration!==generation||ownTopology!==topologyGeneration) return;
    mergeZeroImpedance=prior;topologyBusy=false;error={severity:'error',section:'TOPOLOGY',line:0,message:(e as Error).message??String(e)};restoreView(snapshot);render();
  }
}

async function findPath(event:Event,form:HTMLFormElement) {
  event.preventDefault();if(!snapshot) return;
  fromBus=(form.elements.namedItem('from') as HTMLInputElement).value;toBus=(form.elements.namedItem('to') as HTMLInputElement).value;
  if(topologyBusy) return;
  const ownGeneration=generation,ownPath=++pathGeneration;
  pathBusy=true;pathError=false;pathText='Finding the shortest equipment path…';render();
  try {
    const result=await engine.path(Number(fromBus),Number(toBus));
    if(ownGeneration!==generation||ownPath!==pathGeneration) return;
    path=result;pathBusy=false;
    if(!result) {pathText=`No active AC path connects bus ${fromBus} to bus ${toBus}.`;pathError=false;}
    else {pathText=`${result.hops} equipment hop${result.hops===1?'':'s'} · ${result.buses.join(' → ')}`;setView('path');selectGraph(focusedGraph(snapshot!.graph,new Set(result.node_ids)),`Path · ${fromBus} → ${toBus}`);}
    render();
  } catch(e) {
    if(ownGeneration!==generation||ownPath!==pathGeneration) return;
    path=null;pathBusy=false;pathError=true;pathText=(e as Error).message??String(e);render();
  }
}

async function download(data:string,type:string,extension:string,kind='topology'):Promise<void> {
  const ownGeneration=generation;
  const exportFilename=`${filename.replace(/\.raw$/i,'')}-${kind}.${extension}`;
  try {
    await saveExport(data,type,exportFilename);
  } catch(e) {
    if(ownGeneration!==generation) return;
    error={severity:'error',section:'EXPORT',line:0,message:e instanceof Error?e.message:String(e)};
    render();
  }
}

app.addEventListener('click',event=>{
  const element=(event.target as Element).closest<HTMLElement>('[data-action]');if(!element) return;
  const action=element.dataset.action;
  if(topologyBusy&&action!=='choose') return;
  if(action==='choose') fileInput.click();
  else if(action==='tab') {if(!snapshot||element.matches('.nav-item:disabled')) return;tab=element.dataset.tab as Tab;query='';page=0;render();}
  else if(action==='fit') network.fit();
  else if(action==='inspect-bus') selectInspection({bus:nodeForBus(snapshot!.graph,Number(element.dataset.bus))?.bus??Number(element.dataset.bus),equipment:null});
  else if(action==='source-bus') {const source=snapshot!.case.buses.find(b=>b.number===Number(element.dataset.bus));if(source){selectedBus=source.number;selectedEquipment=null;sourceRecordBus=source.number;network.setSelection({bus:null,equipment:null});const legend=document.querySelector<HTMLElement>('.legend');if(legend&&graphData)legend.innerHTML=graphLegend(graphData,snapshot!,path,{bus:null,equipment:null}).map(entry=>`<span class="legend-${entry.key}"><i aria-hidden="true"></i>${entry.label}</span>`).join('');document.querySelector<HTMLElement>('#inspector .inspector-content')!.innerHTML=inspectorHtml();document.querySelector<HTMLElement>('#inspector h2')?.focus();renderSelection();}}
  else if(action==='inspect-equipment') selectInspection({bus:null,equipment:element.dataset.equipment!});
  else if(action==='full'&&snapshot&&!topologyBusy) {clearPathSelection();selectedIsland=null;selectedNeighborhoodBus=null;setView('full');selectGraph(snapshot.graph,'Full network');render();}
  else if(action==='neighborhood'&&snapshot&&!topologyBusy) {const input=document.querySelector<HTMLInputElement>('#neighborhood-hops')!;if(!input.checkValidity()){input.reportValidity();return;}neighborhoodHops=input.valueAsNumber;clearPathSelection();selectedNeighborhoodBus=Number(element.dataset.bus);selectedBus=nodeForBus(snapshot.graph,selectedNeighborhoodBus)?.bus??selectedNeighborhoodBus;selectedEquipment=null;selectedIsland=null;setView('neighborhood');selectGraph(neighborhood(snapshot.graph,selectedNeighborhoodBus,neighborhoodHops),`Bus ${selectedNeighborhoodBus} · ${neighborhoodHops}-hop neighborhood`);render();}
  else if(action==='neighborhood-csv'&&snapshot&&!topologyBusy) {const input=document.querySelector<HTMLInputElement>('#neighborhood-hops')!;if(!input.checkValidity()){input.reportValidity();return;}neighborhoodHops=input.valueAsNumber;const bus=Number(element.dataset.bus);const selected=neighborhood(snapshot.graph,bus,neighborhoodHops);const ids=new Set(selected.nodes.map(node=>node.id));const rows=busRows().filter(row=>ids.has(`bus:${row.bus}`)&&matchesBus(snapshot!,Number(row.bus),fieldControls.filters));void download(toCsv(rows),'text/csv;charset=utf-8','csv',`bus-${bus}-${neighborhoodHops}-hops`);}
  else if(action==='island'&&snapshot&&!topologyBusy) {const island=snapshot.analysis.islands.find(i=>i.id===Number(element.dataset.island))!;clearPathSelection();selectedIsland=island.id;selectedIslandAnchor=island.buses[0]??null;selectedNeighborhoodBus=null;setView('island');selectGraph(focusedGraph(snapshot.graph,new Set([...island.buses.map(b=>nodeForBus(snapshot!.graph,b)?.id??`bus:${b}`),...island.synthetic_nodes])),`Island ${island.id}`);render();}
  else if(action==='previous'||action==='next') {page+=action==='next'?1:-1;renderTable();}
  else if(action==='csv'&&snapshot) void download(toCsv(filteredRows()),'text/csv;charset=utf-8','csv');
  else if(action==='json'&&snapshot&&!topologyBusy) void download(JSON.stringify({format_version:1,filename,case:snapshot.case,graph:snapshot.graph,merge_zero_impedance:snapshot.graph.merge_zero_impedance,analysis:snapshot.analysis,path,view:{scope:graphView,title:graphTitle,filters:fieldControls.filters,colors:fieldControls.colors,displayed_graph:graphData}},null,2),'application/json','json');
});
app.addEventListener('input',event=>{if((event.target as HTMLElement).id==='result-search'){query=(event.target as HTMLInputElement).value;page=0;renderTable();}});



workspace.addEventListener('dragover',event=>{event.preventDefault();workspace.classList.add('dragging');});
workspace.addEventListener('dragleave',event=>{if(!workspace.contains(event.relatedTarget as Node)) workspace.classList.remove('dragging');});
workspace.addEventListener('drop',event=>{event.preventDefault();workspace.classList.remove('dragging');const file=event.dataTransfer?.files[0];if(file) void openFile(file);});
window.addEventListener('pagehide',()=>{engine.dispose();network.destroy();});
render();
