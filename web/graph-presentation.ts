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

export function graphStyles():StylesheetJson {
  return [
    {selector:'node',style:{'background-color':'#5b7f91','width':25,'height':25,'label':'data(label)','color':'#233c50','font-size':11,'text-valign':'bottom','text-margin-y':6,'border-width':2,'border-color':'#ffffff','min-zoomed-font-size':9}},
    {selector:'node.junction',style:{'shape':'diamond','background-color':'#ffffff','border-color':'#78909f','width':18,'height':18}},
    {selector:'edge',style:{'width':1.7,'line-color':'#94a8b5','curve-style':'bezier'}},
    {selector:'edge.transformer',style:{'line-style':'dashed'}},
    {selector:'edge.switch',style:{'line-style':'dotted'}},
    {selector:'node.bridge, node.articulation',style:{'underlay-color':'#986315','underlay-opacity':0.48,'underlay-padding':5}},
    {selector:'edge.bridge',style:{'underlay-color':'#986315','underlay-opacity':0.65,'underlay-padding':4}},
    {selector:'node.selected',style:{'border-color':'#0f766e','border-width':4}},
    {selector:'edge.selected',style:{'line-outline-color':'#0f766e','line-outline-width':2}},
    {selector:'node.path',style:{'background-color':'#2563eb'}},
    {selector:'edge.path',style:{'line-color':'#2563eb','width':4}},
    {selector:'node.selected, node.path, node.bridge, node.articulation',style:{'min-zoomed-font-size':0}},
    {selector:'node.hovered',style:{'border-style':'dashed','border-width':3}},
    {selector:'edge.hovered',style:{'width':5}},
    {selector:'node.label-hidden',style:{'label':''}},
  ];
}
