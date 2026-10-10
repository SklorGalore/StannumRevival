import cytoscape, {type Core} from 'cytoscape';
import type {Snapshot,GraphData,Path} from './types';

export class NetworkGraph {
  private cy:Core|null=null;
  destroy() {this.cy?.destroy();this.cy=null;}
  fit() {this.cy?.fit(undefined,35);}
  render(container:HTMLElement,graph:GraphData,snapshot:Snapshot,path:Path|null,onInspect:(bus:number|null,equipment:string|null)=>void) {
    this.destroy();
    const bridges=new Set(snapshot.analysis.bridges.map(e=>e.id));
    const cuts=new Set(snapshot.analysis.articulation_buses);
    const pathNodes=new Set(path?.node_ids??[]);
    const pathEdges=new Set(path?.connection_ids??[]);
    const equipment=new Map(snapshot.case.equipment.map(e=>[e.id,e]));
    this.cy=cytoscape({container,
      elements:[
        ...graph.nodes.map(n=>({data:{id:n.id,label:n.bus===null?'T':n.member_buses.length>1?`${n.bus} · ${n.member_buses.length}`:String(n.bus),bus:n.bus,equipment:n.equipment_id},classes:[n.bus===null?'junction':'bus',n.bus!==null&&cuts.has(n.bus)?'cut':'',pathNodes.has(n.id)?'path':''].join(' ')})),
        ...graph.edges.map(e=>({data:{id:e.id,source:e.source,target:e.target,equipment:e.equipment_id},classes:[equipment.get(e.equipment_id)?.kind??'line',bridges.has(e.id)?'bridge':'',pathEdges.has(e.id)?'path':''].join(' ')})),
      ],
      style:[
        {selector:'node',style:{'background-color':'#50738b','width':25,'height':25,'label':'data(label)','color':'#30485c','font-size':10,'text-valign':'bottom','text-margin-y':6,'border-width':2,'border-color':'#fff'}},
        {selector:'node.junction',style:{'shape':'diamond','background-color':'#fff','border-color':'#7f98ab','width':17,'height':17}},
        {selector:'node.cut',style:{'background-color':'#df8b3c','border-color':'#efc89e','border-width':4}},
        {selector:'edge',style:{'width':1.5,'line-color':'#bccbd5','curve-style':'bezier'}},
        {selector:'edge.transformer',style:{'line-style':'dashed','line-color':'#8f78ad'}},
        {selector:'edge.switch',style:{'line-style':'dotted'}},
        {selector:'edge.bridge',style:{'width':2.5,'line-color':'#da8434'}},
        {selector:'node.path',style:{'background-color':'#238b7c','border-color':'#b7ddd3','border-width':4}},
        {selector:'edge.path',style:{'line-color':'#238b7c','width':4}},
        {selector:':selected',style:{'border-color':'#182f43','border-width':3,'line-color':'#182f43'}},
      ],
      layout:{name:graph.nodes.length>180?'circle':'cose',animate:false,padding:40,randomize:false},
      minZoom:.08,maxZoom:4,wheelSensitivity:.25,
    });
    this.cy.on('tap','node',event=>{const n=event.target;onInspect(n.data('bus')??null,n.data('equipment')??null);});
    this.cy.on('tap','edge',event=>onInspect(null,event.target.data('equipment')));
  }
}
