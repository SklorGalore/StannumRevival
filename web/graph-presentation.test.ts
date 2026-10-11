import {describe,expect,it} from 'vitest';
import {graphLegend} from './graph-presentation';
import {NetworkGraph} from './graph';
import type {ColorPresentation} from './field-colors';
import type {GraphData,Snapshot} from './types';

const graph:GraphData={
  nodes:[
    {id:'bus:1',label:'1',bus:1,equipment_id:null,base_kv:230,member_buses:[1]},
    {id:'junction:T1',label:'T',bus:null,equipment_id:'T1',base_kv:null,member_buses:[]},
    {id:'bus:2',label:'2',bus:2,equipment_id:null,base_kv:115,member_buses:[2]},
  ],
  edges:[{id:'connection:T1:1',equipment_id:'T1',source:'bus:1',target:'junction:T1',winding:1},{id:'connection:T1:2',equipment_id:'T1',source:'junction:T1',target:'bus:2',winding:2}],
  zero_impedance_groups:[],merge_zero_impedance:false,absorbed_equipment_ids:[],
};
const snapshot={graph,case:{equipment:[]},analysis:{bridges:[graph.edges[0]],articulation_buses:[1],synthetic_articulations:['junction:T1']}} as unknown as Snapshot;

describe('graph presentation',()=>{
  it('legend reflects only displayed states',()=>{
    const excluded={...graph,nodes:graph.nodes.filter(node=>node.id==='bus:2'),edges:[]};
    expect(graphLegend(excluded,snapshot,null,{bus:99,equipment:'missing'})).toEqual([{key:'bus',label:'Bus'}]);
    expect(graphLegend(graph,snapshot,null,{bus:99,equipment:'missing'})).toEqual([
      {key:'bus',label:'Bus'},
      {key:'junction',label:'Transformer junction'},
      {key:'bridge',label:'Topology bridge'},
      {key:'articulation',label:'Articulation bus or junction'},
    ]);
    expect(graphLegend({...graph,nodes:graph.nodes.filter(node=>node.bus===null),edges:[]},snapshot,null,{bus:null,equipment:null})).toEqual([
      {key:'junction',label:'Transformer junction'},
      {key:'articulation',label:'Articulation junction'},
    ]);
    expect(graphLegend(graph,snapshot,{buses:[1,2],equipment_ids:['T1'],node_ids:['bus:1'],connection_ids:['connection:T1:1'],hops:1},{bus:1,equipment:'T1'}).map(entry=>entry.key)).toEqual(['bus','junction','selected','path','bridge','articulation']);
  });

  it('path and findings remain visible under selection in Cytoscape computed styles',()=>{
    const network=new NetworkGraph();
    network.render(undefined,graph,snapshot,{buses:[1,2],equipment_ids:['T1'],node_ids:['bus:1','junction:T1'],connection_ids:['connection:T1:1'],hops:1},()=>{});
    network.setSelection({bus:1,equipment:'T1'});
    const cy=(network as unknown as {cy:import('cytoscape').Core}).cy;
    const selectedArticulation=cy.getElementById('bus:1');
    const selectedBridge=cy.getElementById('connection:T1:1');
    expect(selectedArticulation.style('border-color')).toBe('rgb(15,118,110)');
    expect(selectedArticulation.style('background-color')).toBe('rgb(37,99,235)');
    expect(selectedArticulation.style('underlay-color')).toBe('rgb(152,99,21)');
    expect(selectedArticulation.style('min-zoomed-font-size')).toBe('0px');
    expect(selectedBridge.style('line-color')).toBe('rgb(37,99,235)');
    expect(selectedBridge.style('underlay-color')).toBe('rgb(152,99,21)');
    expect(selectedBridge.style('line-outline-color')).toBe('rgb(15,118,110)');
    expect(selectedBridge.style('line-outline-width')).toBe('2px');
    expect(cy.getElementById('junction:T1').style('shape')).toBe('diamond');
    network.destroy();
  });

  it('selection updates merged members and all equipment without changing viewport',()=>{
    const merged={...graph,nodes:graph.nodes.map(node=>node.id==='bus:1'?{...node,member_buses:[1,2]}:node)};
    const mergedSnapshot={...snapshot,graph:merged} as Snapshot;
    const network=new NetworkGraph();
    network.render(undefined,merged,mergedSnapshot,{buses:[1,2],equipment_ids:['T1'],node_ids:['bus:1','junction:T1'],connection_ids:['connection:T1:1','connection:T1:2'],hops:1},()=>{});
    const cy=(network as unknown as {cy:import('cytoscape').Core}).cy;
    cy.zoom(1.4);
    cy.pan({x:23,y:41});
    const zoom=cy.zoom();
    const pan=cy.pan();
    network.setSelection({bus:2,equipment:'T1'});
    expect(cy.getElementById('bus:1').hasClass('selected')).toBe(true);
    expect(cy.getElementById('junction:T1').hasClass('selected')).toBe(true);
    expect(cy.edges().toArray().every(edge=>edge.hasClass('selected'))).toBe(true);
    expect(cy.zoom()).toBe(zoom);
    expect(cy.pan()).toEqual(pan);
    cy.zoom(.4);
    expect(cy.getElementById('bus:1').hasClass('label-hidden')).toBe(false);
    expect(cy.getElementById('junction:T1').hasClass('label-hidden')).toBe(false);
    expect(cy.getElementById('bus:2').hasClass('label-hidden')).toBe(true);
    const lowZoom=cy.zoom();
    const lowZoomPan=cy.pan();
    network.setSelection({bus:99,equipment:'absent'});
    expect(cy.elements('.selected')).toHaveLength(0);
    expect(cy.zoom()).toBe(lowZoom);
    expect(cy.pan()).toEqual(lowZoomPan);
    network.destroy();
  });

  it('applies field colors beneath path, finding, and selection styles',()=>{
    const network=new NetworkGraph();
    const presentation:ColorPresentation={nodeColors:{'bus:1':'#ff0000'},edgeColors:{'connection:T1:1':'#00aa00'},legends:[]};
    network.render(undefined,graph,snapshot,{buses:[1],equipment_ids:['T1'],node_ids:['bus:1'],connection_ids:['connection:T1:1'],hops:1},()=>{},presentation);
    network.setSelection({bus:1,equipment:'T1'});
    const cy=(network as unknown as {cy:import('cytoscape').Core}).cy;
    expect(cy.getElementById('bus:1').data('fieldColor')).toBe('#ff0000');
    expect(cy.getElementById('bus:1').style('background-color')).toBe('rgb(37,99,235)');
    expect(cy.getElementById('bus:1').style('border-color')).toBe('rgb(15,118,110)');
    expect(cy.getElementById('bus:1').style('underlay-color')).toBe('rgb(152,99,21)');
    expect(cy.getElementById('connection:T1:1').data('fieldColor')).toBe('#00aa00');
    expect(cy.getElementById('connection:T1:1').style('line-color')).toBe('rgb(37,99,235)');
    expect(cy.getElementById('junction:T1').style('background-color')).toBe('rgb(255,255,255)');
    network.destroy();
  });

  it('recolors an existing graph without resetting viewport and preserves colors on theme changes',()=>{
    const network=new NetworkGraph();
    network.render(undefined,graph,snapshot,null,()=>{}, {nodeColors:{'bus:1':'#ff0000'},edgeColors:{},legends:[]} as ColorPresentation);
    const cy=(network as unknown as {cy:import('cytoscape').Core}).cy;
    cy.zoom(1.3);
    cy.pan({x:23,y:41});
    network.setColors({nodeColors:{'bus:1':'#00ff00'},edgeColors:{'connection:T1:1':'#0000ff'},legends:[]});
    expect(cy.getElementById('bus:1').style('background-color')).toBe('rgb(0,255,0)');
    expect(cy.getElementById('connection:T1:1').style('line-color')).toBe('rgb(0,0,255)');
    expect(cy.zoom()).toBe(1.3);
    expect(cy.pan()).toEqual({x:23,y:41});
    network.updateTheme();
    expect(cy.getElementById('bus:1').style('background-color')).toBe('rgb(0,255,0)');
    expect(cy.getElementById('connection:T1:1').style('line-color')).toBe('rgb(0,0,255)');
    network.destroy();
  });
});
