import { describe,it,expect } from 'vitest';
import {focusedGraph,neighborhood,toCsv} from './view-model';
import type {GraphData} from './types';
const graph:GraphData = {
  nodes:[1,2,3,4].map(bus=>({id:`bus:${bus}`,label:String(bus),bus,equipment_id:null,base_kv:138,member_buses:[bus]})),
  edges:[['a',1,2],['b',2,3],['c',3,4]].map(([id,a,b])=>({id:String(id),source:`bus:${a}`,target:`bus:${b}`,equipment_id:String(id),winding:null})),
  zero_impedance_groups:[],merge_zero_impedance:false,absorbed_equipment_ids:[],
};
describe('focused graph',()=>{
  it('includes only selected nodes and connections entirely inside the selection',()=>{
    const result=focusedGraph(graph,new Set(['bus:1','bus:2']));
    expect(result.nodes.map(n=>n.bus)).toEqual([1,2]);
    expect(result.edges.map(e=>e.id)).toEqual(['a']);
  });
  it('retains graph mode metadata when focusing a subgraph',()=>{
    const merged={...graph,merge_zero_impedance:true,absorbed_equipment_ids:['switch:1']};
    const result=focusedGraph(merged,new Set(['bus:1']));
    expect(result.merge_zero_impedance).toBe(true);
    expect(result.absorbed_equipment_ids).toEqual(['switch:1']);
  });
  it('limits neighborhoods by equipment hops',()=>{
    expect(neighborhood(graph,1,2).nodes.map(n=>n.bus)).toEqual([1,2,3]);
    expect(neighborhood(graph,1,8).nodes.map(n=>n.bus)).toEqual([1,2,3,4]);
    const star:GraphData={...graph,nodes:[...graph.nodes,{id:'junction:t',label:'t',bus:null,equipment_id:'t',base_kv:null,member_buses:[]}],edges:[1,2,3].map(bus=>({id:`t${bus}`,source:`bus:${bus}`,target:'junction:t',equipment_id:'t',winding:bus}))};
    expect(neighborhood(star,1,1).nodes.map(n=>n.id)).toEqual(['bus:1','bus:2','bus:3','junction:t']);
  });
  it('resolves a member bus to its canonical neighborhood node',()=>{
    const grouped:GraphData={...graph,nodes:[{...graph.nodes[0],member_buses:[1,5]},...graph.nodes.slice(1)],zero_impedance_groups:[[1,5]],merge_zero_impedance:true};
    expect(neighborhood(grouped,5,0).nodes.map(n=>n.id)).toEqual(['bus:1']);
  });
});
describe('CSV export',()=>{
  it('escapes delimiters quotes and newlines and neutralizes spreadsheet formulas',()=>{
    expect(toCsv([{bus:1,name:'A, "B"\nC'},{bus:2,name:'=1+1'}])).toBe('bus,name\r\n1,"A, ""B""\nC"\r\n2,\'=1+1\r\n');
  });
});
