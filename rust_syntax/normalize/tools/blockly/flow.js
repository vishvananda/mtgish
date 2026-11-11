/* Compact control blocks retain the exact instruction/operation boundaries. */
'use strict';
function createFlowView(api) {
  const instructionFamilies=[['If','Optional'],['ChooseOne'],['ForEachObject','ForEachPlayer'],['Repeatable','Times'],['Group']];
  const values=['BindValue','NoteObjects','JoinResults'];
  const labels={If:'If',Optional:'May do',ChooseOne:'Choose one option',ForEachObject:'For each object',ForEachPlayer:'For each player',Repeatable:'Repeatable process',Times:'Repeatable process · capped',Group:'Sequence',RepeatProcess:'Run this process again',BindValue:'Remember number',NoteObjects:'Remember objects',JoinResults:'Combine saved results',Together:'Together · one result',InOrder:'In order · one result',Repeat:'Repeat action'};
  function variants(s,d){
    if(s.ty.named==='Instruction'&&s.pr!=='effect'){const family=instructionFamilies.find(f=>f.includes(s.variant));if(family)return d.variants.filter(v=>family.includes(v.wire));}
    if(s.pr==='effect'&&api.actions[s.op]?.control){const family=values.includes(s.op)?values:s.op==='RepeatProcess'?['RepeatProcess']:Object.keys(api.actions).filter(n=>api.actions[n].control&&!values.includes(n)&&n!=='RepeatProcess');return d.variants.filter(v=>family.includes(v.wire));}
    return null;
  }
  function parts(s,list){return list.map(p=>{
    if(s.compactFlow&&s.ty.named==='Instruction'&&s.variant==='ChooseOne'&&p.key==='F_options')p={...p,flow:'array',socketTy:{named:'Program'},statement:true,label:'options'};
    if(s.flowOption&&p.key==='F_instructions')p={...p,flow:'array',socketTy:{named:'Instruction'},statement:true,label:'do'};
    if(s.compactFlow&&p.ty.vec?.named==='Operation')p={...p,flow:'array',socketTy:{named:'Operation'},statement:true,label:'actions'};
    if(s.ty.named==='Instruction'){
      if(p.key==='F_condition')p={...p,label:''};
      if(['F_objects','F_players','F_amount'].includes(p.key))p={...p,label:p.key==='F_amount'?'maximum runs':''};
      if(p.ty.named==='Program')p={...p,label:p.key==='F_then'?'then':p.key==='F_otherwise'?'otherwise':'do'};
      if(p.key==='F_decider')p={...p,label:'chosen by'};
    }
    if(s.ty.named==='Condition'&&s.variant==='Compare')p={...p,label:''};
    return p;
  });}
  const compact=s=>s.flowOption||s.ty.named==='Instruction'&&s.pr!=='effect'||s.ty.named==='Condition'&&s.variant==='Compare';
  function refresh(b){if(!b.state.flowOption)return;let n=1,prev=b.getPreviousBlock();while(prev?.state.flowOption){n++;prev=prev.getPreviousBlock();}b.getField('OPTION_TITLE')?.setValue('Option '+n);}
  function addOption(b){if(b.isInFlyout)return;api.remember();api.addToFlow(b,api.parts(b.state).find(p=>p.key==='F_options'),{instructions:[]});}
  function starter(name){
    const value=api.instruction(name),args=value.args;
    if(name==='ChooseOne')args.options=[{instructions:[]},{instructions:[]}];
    if(name==='If')args.condition={kind:'Compare',args:{left:{kind:'Constant',args:1},comparison:'Equal',right:{kind:'Constant',args:1}}};
    if(name==='ForEachObject')args.objects={kind:'Zone',args:{scope:{Shared:'Battlefield'},kind:'Permanent',filter:{kind:'All',args:[{kind:'CardType',args:'Creature'},{kind:'ControlledBy',args:{kind:'One',args:{kind:'AbilityController'}}}]}}};
    if(name==='ForEachPlayer')args.players={kind:'All'};
    if(name==='Times')args.amount={kind:'Constant',args:2};
    return value;
  }
  function palette(){return [
    {kind:'label',text:'Conditions & choices'},api.block('If',starter('If')),api.block('ChooseOne',starter('ChooseOne')),
    {kind:'label',text:'Loops & repeating'},api.block('ForEachObject',starter('ForEachObject')),api.block('Repeatable',starter('Repeatable')),api.effect('RepeatProcess'),
    {kind:'label',text:'Sequence & saved values'},api.block('Group',starter('Group')),api.effect('BindValue',{value:{kind:'Constant',args:1}})
  ];}
  return {labels,variants,parts,compact,refresh,addOption,starter,palette};
}
