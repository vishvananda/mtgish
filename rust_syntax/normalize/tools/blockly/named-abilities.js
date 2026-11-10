/* Named activated abilities are complete recipes, not identity dropdowns. */
'use strict';
function createNamedAbilities({abilityValue,actionValue,clone,same}) {
  const source={kind:'Source'},you={kind:'AbilityController'},one={kind:'One',args:source},constant=n=>({kind:'Constant',args:n});
  function cycling() {
    const a=abilityValue('Activated');
    a.args.origin={Cycling:{search:null}};
    a.args.cost.parts=[{Mana:[{Generic:2}]},{Discard:{count:constant(1),filter:{kind:'SameObject',args:source},random:false}}];
    a.args.body.instructions=[actionValue('Draw',{players:{kind:'One',args:you},amount:constant(1)})];
    return {kind:'Scoped',args:{zones:['Hand'],condition:null,ability:a}};
  }
  function equip() {
    const a=abilityValue('Activated');a.args.origin='Equip';a.args.cost.parts=[{Mana:[{Generic:1}]}];a.args.limits.sorcery=true;
    a.args.announcement.targets=[{slot:0,kind:{Object:{kind:'Permanent',zones:['Battlefield']}},count:{Exactly:constant(1)},filter:{kind:'All',args:[{kind:'CardType',args:'Creature'},{kind:'ControlledBy',args:{kind:'One',args:you}}]},players:null,distribute:null,constraints:[],chooser:null}];
    a.args.body.instructions=[actionValue('Attach',{attachments:one,host:{kind:'Target',args:{slot:0,index:0}}})];return a;
  }
  function crew() {
    const a=abilityValue('Activated');a.args.origin='Crew';
    a.args.cost.parts=[{Constrained:{part:{ChangeTapState:{selections:[{objects:{kind:'Zone',args:{scope:{Shared:'Battlefield'},kind:'Permanent',filter:{kind:'All',args:[{kind:'CardType',args:'Creature'},{kind:'Not',args:{kind:'Tapped'}},{kind:'ControlledBy',args:{kind:'One',args:you}},{kind:'Not',args:{kind:'SameObject',args:source}}]}}},choose:'AnyNumber'}],tapped:true,symbol:false}},constraints:[{TotalProperty:{property:'Power',comparison:'GreaterOrEqual',amount:constant(1)}}]}}];
    a.args.body.instructions=[actionValue('CreateContinuousEffect',{source,affected:{Objects:one},changes:[{kind:'AddCardType',args:'Artifact'},{kind:'AddCardType',args:'Creature'}],duration:'EndOfTurn',evaluation:'Snapshot'})];return a;
  }
  function unscoped(a){while(a&&['Scoped','WithPresentation'].includes(a.kind))a=a.args.ability;return a;}
  function name(a) {
    a=unscoped(a);if(a?.kind!=='Activated')return null;
    if(['Equip','Crew'].includes(a.args.origin))return a.args.origin;
    if(a.args.origin?.Cycling)return a.args.origin.Cycling.search===null?'Cycling':'Typecycling';return null;
  }
  function bodySame(a,expected){const list=a.args.body.instructions;return list.length===1&&list[0].kind==='Effect'&&same(list[0].args.operation,expected.args.body.instructions[0].args.operation);}
  function standard(a) {
    a=unscoped(a);const title=name(a);if(!title)return null;
    const expected=title==='Equip'?equip():title==='Crew'?crew():title==='Cycling'?unscoped(cycling()):null;
    if(!expected)return {name:title,hidden:[],hint:'Discard this card to use its typecycling ability.'};
    if(title==='Equip') {
      const slot=a.args.announcement.targets[0]?.slot;
      if(slot!==undefined){expected.args.announcement.targets[0].slot=slot;expected.args.body.instructions[0].args.operation.args.host.args.slot=slot;}
    }
    const hidden=[];
    if(same(a.args.limits,expected.args.limits))hidden.push('F_limits');
    if(same(a.args.announcement,expected.args.announcement))hidden.push('F_announcement');
    if(bodySame(a,expected))hidden.push('F_body');
    const hints={Equip:'Attach to a creature you control · sorcery speed',Crew:'Tap other creatures · becomes a creature this turn',Cycling:'Discard this card → draw a card · hand'};
    return {name:title,hidden,hint:hidden.length===3?hints[title]:'Additional rules are shown below.'};
  }
  function crewPower(part) {
    const amount=part?.Constrained?.constraints?.[0]?.TotalProperty?.amount;
    if(amount?.kind!=='Constant')return undefined;
    const test=clone(part);test.Constrained.constraints[0].TotalProperty.amount=constant(1);
    return same(test,crew().args.cost.parts[0])?amount.args:undefined;
  }
  const discard={Discard:{count:constant(1),filter:{kind:'SameObject',args:source},random:false}};
  return {cycling,equip,crew,name,standard,crewPower,discard,unscoped};
}
