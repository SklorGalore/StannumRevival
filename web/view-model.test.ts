import { describe,it,expect } from 'vitest';
import {focusedGraph,neighborhood,toCsv} from './view-model';
import type {GraphData} from './types';
const graph:GraphData = {
  nodes:[1,2,3,4].map(bus=>({id:`bus:${bus}`,label:String(bus),bus,equipment_id:null,base_kv:138})),
  edges:[['a',1,2],['b',2,3],['c',3,4]].map(([id,a,b])=>({id:String(id),source:`bus:${a}`,target:`bus:${b}`,equipment_id:String(id),winding:null})),
};
describe('focused graph',()=>{
  it('includes only selected nodes and connections entirely inside the selection',()=>{
    const result=focusedGraph(graph,new Set(['bus:1','bus:2']));
    expect(result.nodes.map(n=>n.bus)).toEqual([1,2]);
    expect(result.edges.map(e=>e.id)).toEqual(['a']);
  });
  it('limits neighborhoods by equipment hops',()=>{
    expect(neighborhood(graph,1,2).nodes.map(n=>n.bus)).toEqual([1,2,3]);
    const star:GraphData={nodes:[...graph.nodes,{id:'junction:t',label:'t',bus:null,equipment_id:'t',base_kv:null}],edges:[1,2,3].map(bus=>({id:`t${bus}`,source:`bus:${bus}`,target:'junction:t',equipment_id:'t',winding:bus}))};
    expect(neighborhood(star,1,1).nodes.map(n=>n.id)).toEqual(['bus:1','bus:2','bus:3','junction:t']);
  });
});
describe('CSV export',()=>{
  it('escapes delimiters quotes and newlines and neutralizes spreadsheet formulas',()=>{
    expect(toCsv([{bus:1,name:'A, "B"\nC'},{bus:2,name:'=1+1'}])).toBe('bus,name\r\n1,"A, ""B""\nC"\r\n2,\'=1+1\r\n');
  });
});
