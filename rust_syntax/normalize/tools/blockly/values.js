/* Readable values and tests without changing their Rust wire representation. */
'use strict';
function createValueView(api) {
  const policy=api.policy,binary=policy.binary,variables=policy.variables,labels=policy.labels;
  const isList=(t,name)=>policy.list_types.includes(t)&&policy.list_variants.includes(name);
  const number=n=>({kind:'Constant',args:n});
  const comparison=()=>({kind:'Compare',args:{left:number(1),test:{kind:'Compare',args:{comparison:'Equal',value:number(1)}}}});
  function label(type,name){return ['Expr','Condition','NumberTest'].includes(type)?labels[name]:null;}
  function state(t,value,s){if(isList(t.named,s.variant))s.valueList=value.args.length;return s;}
  function listType(s){return {named:s.ty.named};}
  function parts(s,list){
    if(s.valueList!==undefined)return Array.from({length:s.valueList},(_,i)=>({key:'ITEM_'+i,label:i?(s.variant==='AllOf'?'and':'or'):'',ty:listType(s)}));
    return list.map(p=>{
      if(s.ty.named==='Expr'){
        if(binary.includes(s.variant)&&p.key.startsWith('ARG_'))p={...p,label:''};
        if(s.variant==='Divide')p={...p,label:p.key==='F_rounding'?'rounding':'' ,...(p.key==='F_rounding'?{options:[['down','Down'],['up','Up']]}:{})};
        if(s.variant==='Conditional')p={...p,label:({F_condition:'',F_then:'then',F_otherwise:'else'})[p.key]??p.label};
        if(s.variant==='ObjectProperty')p={...p,label:(Object.fromEntries(Object.entries(policy.property_fields).map(([k,v])=>['F_'+k,v])))[p.key]??p.label};
        if(s.variant==='PlayerNumber')p={...p,label:p.key==='F_player'?'of':''};
        if(s.variant==='CountPlayers')p={...p,label:''};
      }
      if(s.ty.named==='NumberTest'&&s.variant==='Compare')p={...p,label:policy.comparison_fields[p.key.slice(2)]??p.label};
      if(s.ty.named==='Condition'){
        if(s.variant==='Compare'||s.variant==='Not')p={...p,label:''};
        if(s.variant==='PlayerIn')p={...p,label:''};
      }
      return p;
    });
  }
  function variants(s,d){
    const family=s.ty.named==='Expr'?(binary.includes(s.variant)?binary:variables.includes(s.variant)?variables:['Divide','Conditional','ObjectProperty','PlayerNumber','CountPlayers'].includes(s.variant)?[s.variant]:null):s.ty.named==='NumberTest'?d.variants.map(v=>v.wire):s.ty.named==='Condition'?(['AllOf','AnyOf'].includes(s.variant)?['AllOf','AnyOf']:['Compare','Not','PlayerIn','PlayerIsActive','ObjectMatches','PlayerState','Window'].includes(s.variant)?[s.variant]:null):null;
    return family?d.variants.filter(v=>family.includes(v.wire)):null;
  }
  function compact(s){return s.valueList!==undefined?s.valueList<=3:['Expr','Condition','NumberTest'].includes(s.ty.named)&&!!labels[s.variant];}
  function decorate(b){
    const s=b.state;if(s.ty.named==='Expr'&&binary.includes(s.variant))b.moveInputBefore('HEADER','ARG_1');
    if(s.ty.named==='Expr'&&s.variant==='Divide')b.moveInputBefore('HEADER','F_denominator');
    if(s.ty.named==='Expr'&&s.variant==='ObjectProperty')b.moveInputBefore('F_property','F_object');
    if(s.ty.named==='Expr'&&s.variant==='PlayerNumber')b.moveInputBefore('F_number','F_player');
    if(s.ty.named==='Condition'&&['PlayerIsActive','PlayerIn','ObjectMatches'].includes(s.variant)){
      const first=api.parts(s)[0]?.key;if(first)b.moveInputBefore(first,'HEADER');
    }
  }
  function refresh(b,details){
    let changed=false;
    const show=(item,visible)=>{if(item&&item.isVisible()!==visible){item.setVisible(visible);changed=true;}};
    if(['Condition','NumberTest'].includes(b.state.ty.named)&&b.state.variant==='Compare')show(b.getInput('HEADER'),details);
    if(b.state.ty.named==='Expr'&&['ObjectProperty','PlayerNumber'].includes(b.state.variant))show(b.getField('VARIANT'),details);
    show(b.getField('REMOVE_VALUE'),b.state.valueList>0);show(b.getField('EMPTY_VALUES'),b.state.valueList===0);
    return changed;
  }
  function reset(){return undefined;}
  function listValues(s,value){return Object.fromEntries(value.args.map((v,i)=>['ITEM_'+i,v]));}
  function listValue(s,values){return {kind:s.variant,args:Array.from({length:s.valueList},(_,i)=>values['ITEM_'+i])};}
  function defaultItem(s){return s.valueList!==undefined?(s.ty.named==='Condition'?comparison():{kind:'Compare',args:{comparison:'Equal',value:number(1)}}):undefined;}
  function starter(type,name){
    if(type==='Expr'){
      if(binary.includes(name))return {kind:name,args:[number(1),number(1)]};
      if(name==='Divide')return {kind:name,args:{numerator:number(1),denominator:number(2),rounding:'Down'}};
      if(name==='Conditional')return {kind:name,args:{condition:comparison(),then:number(1),otherwise:number(0)}};
    }
    if(type==='NumberTest'){
      if(name==='Compare')return {kind:name,args:{comparison:'Equal',value:number(1)}};
      if(['AllOf','AnyOf'].includes(name))return {kind:name,args:[starter(type,'Compare'),starter(type,'Compare')]};
    }
    if(type==='Condition'){
      if(name==='Compare')return comparison();
      if(['AllOf','AnyOf'].includes(name))return {kind:name,args:[comparison(),comparison()]};
      if(name==='Not')return {kind:name,args:comparison()};
    }
    return null;
  }
  function palette(){return [
    {kind:'label',text:'Numbers & arithmetic'},api.block('Expr',number(1)),api.block('Expr',{kind:'AnnouncedX'}),api.block('Expr',starter('Expr','Add')),api.block('Expr',starter('Expr','Divide')),api.block('Expr',starter('Expr','Conditional')),
    {kind:'label',text:'Counts & properties'},api.count(),api.block('Expr',{kind:'ObjectProperty',args:{object:{kind:'Source'},property:'ManaValue',information:'CurrentOrLastKnown'}}),api.block('Expr',{kind:'PlayerNumber',args:{player:{kind:'AbilityController'},number:'Life'}}),
    {kind:'label',text:'Conditions'},api.block('Condition',comparison()),api.block('Condition',starter('Condition','Not')),api.block('Condition',starter('Condition','AllOf')),api.block('NumberTest',starter('NumberTest','AnyOf')),
    {kind:'label',text:'Players'},api.block('PlayerSet',{kind:'All'})
  ];}
  return {label,state,parts,variants,compact,decorate,refresh,reset,listValues,listValue,defaultItem,starter,palette};
}
