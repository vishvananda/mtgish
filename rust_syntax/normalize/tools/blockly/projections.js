/* Presentation conversions are explicit, directional and lossless on import. */
'use strict';
function createAuthoringProjections({types,clone,same}) {
  const edges=[['ObjectRef','ObjectSet','One'],['PlayerRef','PlayerSet','One'],['ObjectSet','EffectSubjects','Objects'],['PlayerSet','EffectSubjects','Players'],['ObjectSet','Recipients','Objects'],['PlayerSet','Recipients','Players']];
  const cards=[['permanents','Any'],['creatures','Creature'],['artifacts','Artifact'],['enchantments','Enchantment'],['lands','Land'],['planeswalkers','Planeswalker'],['battles','Battle']];
  function route(actual,expected) {
    if(JSON.stringify(actual)===JSON.stringify(expected))return [];
    if(!actual?.named||!expected?.named)return null;
    const queue=[[actual.named,[]]],seen=new Set();
    while(queue.length){const [type,path]=queue.shift();if(seen.has(type))continue;seen.add(type);for(const edge of edges)if(edge[0]===type){const next=[...path,edge];if(edge[1]===expected.named)return next;queue.push([edge[1],next]);}}
    return null;
  }
  const compatible=(actual,expected)=>route(actual,expected)!==null;
  function outputs(t){return [t,...[...new Set(edges.map(e=>e[1]))].filter(n=>n!==t.named&&compatible(t,{named:n})).map(named=>({named}))];}
  function peel(t,value) {
    if(!t?.named)return null;
    const d=types[t.named],variant=d?.adjacent?value?.kind:value&&typeof value==='object'?Object.keys(value)[0]:value;
    const edge=edges.find(e=>e[1]===t.named&&e[2]===variant);
    return edge?{ty:{named:edge[0]},value:d.adjacent?value.args:value[variant]}:null;
  }
  function coerce(actual,expected,value) {
    const path=route(actual,expected);if(path===null)throw Error('This block cannot fill the '+(expected.named||'requested')+' socket.');
    for(const [,to,variant] of path)value=types[to].adjacent?{kind:variant,args:value}:{[variant]:value};return value;
  }
  function unwrap(actual,expected,value) {
    const steps=route(actual,expected);if(steps===null)throw Error('Incompatible typed binding port.');let path='';
    for(const [,to,variant] of [...steps].reverse()){
      const d=types[to],key=d.adjacent?'args':variant;
      if((d.adjacent?value?.kind:Object.keys(value||{})[0])!==variant)throw Error('Wrong wrapper at typed binding port.');
      value=value[key];path+='/'+key;
    }
    return {value,path};
  }
  function query(value) {
    if(value?.kind!=='Zone'||value.args.kind!=='Permanent'||!same(value.args.scope,{Shared:'Battlefield'}))return null;
    const filter=value.args.filter,clauses=filter.kind==='All'?filter.args:[filter];let card='Any',control='Any';
    for(const f of clauses){
      if(f.kind==='Any')continue;
      if(f.kind==='CardType'&&card==='Any'&&cards.some(([,v])=>v===f.args)){card=f.args;continue;}
      if(f.kind==='ControlledBy'&&control==='Any'&&same(f.args,{kind:'One',args:{kind:'AbilityController'}})){control='You';continue;}
      return null;
    }
    return {Q_CARD:card,Q_CONTROL:control};
  }
  function queryValue(state,values) {
    if(!cards.some(([,v])=>v===values.Q_CARD)||!['Any','You'].includes(values.Q_CONTROL))throw Error('Choose a valid object selector.');
    if(same(query(state.queryRaw),values))return clone(state.queryRaw);
    const filters=[];if(values.Q_CARD!=='Any')filters.push({kind:'CardType',args:values.Q_CARD});
    if(values.Q_CONTROL==='You')filters.push({kind:'ControlledBy',args:{kind:'One',args:{kind:'AbilityController'}}});
    return {kind:'Zone',args:{scope:{Shared:'Battlefield'},kind:'Permanent',filter:filters.length===1?filters[0]:filters.length?{kind:'All',args:filters}:{kind:'Any'}}};
  }
  return {peel,compatible,outputs,coerce,unwrap,query,queryValue,cards};
}
