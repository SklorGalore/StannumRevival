import type {GraphData,GraphNode,Snapshot} from './types';
import {nodeForBus} from './view-model';

export function busRows(snapshot:Snapshot):Record<string,unknown>[] {
  const {case:c,analysis:a,graph}=snapshot;
  const active=new Set(graph.nodes.filter(n=>n.bus!==null).flatMap(n=>n.member_buses?.length?n.member_buses:[n.bus!]));
  const rows:Record<string,unknown>[]=[];
  for(const node of graph.nodes) {
    if(node.bus===null) continue;
    const bus=c.buses.find(b=>b.number===node.bus);
    if(!bus) continue;
    const degree=a.degrees.find(d=>d.bus===node.bus);
    const members=node.member_buses?.length?node.member_buses:[node.bus];
    rows.push({bus:node.bus,name:bus.name.trim(),...(graph.merge_zero_impedance?{member_buses:members}:{}),base_kv:bus.base_kv,degree:degree?.degree??'',island:degree?.island??'',status:'Active',articulation:a.articulation_buses.includes(node.bus)});
  }
  for(const bus of c.buses) if(!active.has(bus.number)) rows.push({bus:bus.number,name:bus.name.trim(),...(graph.merge_zero_impedance?{member_buses:[bus.number]}:{}),base_kv:bus.base_kv,degree:'',island:'',status:'Excluded',articulation:false});
  return rows;
}

export function memberSearchText(graph:GraphData,buses:Snapshot['case']['buses'],node:GraphNode):string {
  return (node.member_buses??(node.bus===null?[]:[node.bus])).map(number=>`${number} ${buses.find(b=>b.number===number)?.name??''}`).join(' ').toLowerCase();
}

