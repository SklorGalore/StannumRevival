import type {StylesheetJson} from 'cytoscape';
import {nodeForBus} from './view-model';
import type {GraphData,Path,Snapshot} from './types';

export type GraphSelection = {bus:number|null;equipment:string|null};
export type LegendEntry = {key:'bus'|'junction'|'selected'|'path'|'bridge'|'articulation';label:string};

const legendLabels:Record<LegendEntry['key'],string>={
  bus:'Bus',junction:'Transformer junction',selected:'Selected',path:'Path',
  bridge:'Topology bridge',articulation:'Articulation bus',
};

export function graphLegend(graph:GraphData,snapshot:Snapshot,path:Path|null,selection:GraphSelection):LegendEntry[] {
  const nodeIds=new Set(graph.nodes.map(node=>node.id));
  const edgeIds=new Set(graph.edges.map(edge=>edge.id));
  const present:LegendEntry['key'][]=[];
  if(graph.nodes.some(node=>node.bus!==null)) present.push('bus');
  if(graph.nodes.some(node=>node.bus===null)) present.push('junction');
  const selectedBusNode=selection.bus===null?undefined:nodeForBus(graph,selection.bus);
  const selectedEquipmentPresent=selection.equipment!==null&&(graph.edges.some(edge=>edge.equipment_id===selection.equipment)||graph.nodes.some(node=>node.equipment_id===selection.equipment));
  if(selectedBusNode||selectedEquipmentPresent) present.push('selected');
  if(path&&(path.node_ids.some(id=>nodeIds.has(id))||path.connection_ids.some(id=>edgeIds.has(id)))) present.push('path');
  const bridgeIds=new Set(snapshot.analysis.bridges.map(edge=>edge.id));
  if(graph.edges.some(edge=>bridgeIds.has(edge.id))) present.push('bridge');
  const articulation=new Set(snapshot.analysis.articulation_buses);
  const synthetic=new Set(snapshot.analysis.synthetic_articulations);
  const hasArticulationBus=graph.nodes.some(node=>node.bus!==null&&articulation.has(node.bus));
  const hasArticulationJunction=graph.nodes.some(node=>node.bus===null&&synthetic.has(node.id));
  if(hasArticulationBus||hasArticulationJunction) present.push('articulation');
  return present.map(key=>({
    key,
    label:key==='articulation'
      ? hasArticulationBus&&hasArticulationJunction?'Articulation bus or junction':hasArticulationJunction?'Articulation junction':'Articulation bus'
      : legendLabels[key],
  }));
}

export function graphStyles(dark=false):StylesheetJson {
  const colors=dark
    ? {node:'#88a9bc',text:'#e2eaf0',surface:'#182633',junction:'#9bb1c0',edge:'#7893a7',finding:'#efbd66',selection:'#5ee0ca',path:'#80aaff'}
    : {node:'#5b7f91',text:'#233c50',surface:'#ffffff',junction:'#78909f',edge:'#94a8b5',finding:'#986315',selection:'#0f766e',path:'#2563eb'};
  return [
    {selector:'node',style:{'background-color':colors.node,'width':25,'height':25,'label':'data(label)','color':colors.text,'font-size':11,'text-valign':'bottom','text-margin-y':6,'border-width':2,'border-color':colors.surface,'min-zoomed-font-size':9}},
    {selector:'node.bus.field-colored',style:{'background-color':'data(fieldColor)'}},
    {selector:'node.junction',style:{'shape':'diamond','background-color':colors.surface,'border-color':colors.junction,'width':18,'height':18}},
    {selector:'edge',style:{'width':1.7,'line-color':colors.edge,'curve-style':'bezier'}},
    {selector:'edge.field-colored',style:{'line-color':'data(fieldColor)'}},
    {selector:'edge.transformer',style:{'line-style':'dashed'}},
    {selector:'edge.switch',style:{'line-style':'dotted'}},
    {selector:'node.bridge, node.articulation',style:{'underlay-color':colors.finding,'underlay-opacity':0.48,'underlay-padding':5}},
    {selector:'edge.bridge',style:{'underlay-color':colors.finding,'underlay-opacity':0.65,'underlay-padding':4}},
    {selector:'node.selected',style:{'border-color':colors.selection,'border-width':4}},
    {selector:'edge.selected',style:{'line-outline-color':colors.selection,'line-outline-width':2}},
    {selector:'node.path',style:{'background-color':colors.path}},
    {selector:'edge.path',style:{'line-color':colors.path,'width':4}},
    {selector:'node.selected, node.path, node.bridge, node.articulation',style:{'min-zoomed-font-size':0}},
    {selector:'node.hovered',style:{'border-style':'dashed','border-width':3}},
    {selector:'edge.hovered',style:{'width':5}},
    {selector:'node.label-hidden',style:{'label':''}},
  ];
}
