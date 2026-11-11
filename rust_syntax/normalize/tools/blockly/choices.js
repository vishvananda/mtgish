/* Direct target rows and mode cards; full wire operands remain typed and exact. */
'use strict';
function createChoiceView(api) {
  const constant=n=>({kind:'Constant',args:n});
  function targetCount(count) {
    if(count==='AnyNumber')return {T_STYLE:'AnyNumber',T_NUMBER:1};
    const style=Object.keys(count||{})[0],amount=count?.[style];
    return ['Exactly','UpTo','AtLeast'].includes(style)&&amount?.kind==='Constant'?{T_STYLE:style,T_NUMBER:amount.args}:null;
  }
  function modeCount(count) {
    if(count.minimum.kind!=='Constant'||count.maximum.kind!=='Constant')return null;
    const min=count.minimum.args,max=count.maximum.args;return {M_STYLE:min===max?'Exactly':min===0?'UpTo':'Between',M_MIN:min,M_MAX:max};
  }
  function state(t,value,s) {
    if(t.named==='TargetSpec')s.simpleTargetCount=!!targetCount(value.count);
    if(t.named==='Announcement'){s.pr='announcement';s.directModes=value.modes!==null;}
    if(t.named==='ModeSelection'){s.pr='modes';s.simpleModeCount=!!modeCount(value.count);s.impliedModes=value.mechanism==='Implied';}
    if(t.named==='Mode')s.pr='mode';
    return s;
  }
  function parts(s,list) {
    return list.flatMap(p=>{
      if(s.simpleTargetCount&&p.key==='F_count')return [{key:'T_STYLE',label:'',ty:{named:'String'},options:[['exactly','Exactly'],['up to','UpTo'],['any number','AnyNumber'],['at least','AtLeast']]},{key:'T_NUMBER',label:'',ty:{named:'i64'}}];
      if(s.pr==='modes'&&s.simpleModeCount&&p.key==='F_count')return [{key:'M_STYLE',label:'',ty:{named:'String'},options:[['exactly','Exactly'],['up to','UpTo'],['between','Between']]},{key:'M_MIN',label:'',ty:{named:'i64'}},{key:'M_MAX',label:'',ty:{named:'i64'},suffix:'modes'}];
      if(s.pr==='announcement'&&p.key==='F_targets')return [{...p,flow:'array',socketTy:{named:'TargetSpec'},statement:true,label:'targets'}];
      if(s.pr==='announcement'&&s.directModes&&p.key==='F_modes')return [{...p,ty:{named:'ModeSelection'},label:'modes'}];
      if(s.pr==='modes'&&p.key==='F_options')return [{...p,flow:'array',socketTy:{named:'Mode'},statement:true,label:'modes'}];
      if(s.pr==='mode'&&p.key==='F_body')p={...p,label:'effects'};
      if(s.pr==='mode'&&p.key==='F_weight')p={...p,label:'mode cost'};
      if(s.pr==='modes'&&p.key==='F_budget')p={...p,label:'point budget'};
      return [p];
    });
  }
  function values(s,value,result) {
    if(s.simpleTargetCount)Object.assign(result,targetCount(value.count));
    if(s.pr==='modes'&&s.simpleModeCount)Object.assign(result,modeCount(value.count));
    return result;
  }
  function fields(s,values) {
    if(s.simpleTargetCount){const n=values.T_NUMBER;if(n<0)throw Error('Target count cannot be negative.');if(!['Exactly','UpTo','AnyNumber','AtLeast'].includes(values.T_STYLE))throw Error('Choose a target count.');values.F_count=values.T_STYLE==='AnyNumber'?'AnyNumber':{[values.T_STYLE]:constant(n)};}
    if(s.pr==='modes'&&s.simpleModeCount){const max=values.M_MAX,min=values.M_STYLE==='Exactly'?max:values.M_STYLE==='UpTo'?0:values.M_MIN;if(!['Exactly','UpTo','Between'].includes(values.M_STYLE)||min<0||max<min)throw Error('Mode bounds must be nonnegative, with minimum no greater than maximum.');values.F_count={minimum:constant(min),maximum:constant(max)};}
    return values;
  }
  const managed=(s,p)=>s.simpleTargetCount&&p.key.startsWith('T_')||s.pr==='modes'&&p.key.startsWith('M_');
  function hidden(b,p) {
    if(p.key==='F_body'&&b.state.ty.named==='Ability'&&!b.getInputTargetBlock(p.key))try{if(api.fromBlock(b).args.announcement?.modes)return true;}catch(_){}
    if(b.state.simpleTargetCount&&p.key==='T_NUMBER')return b.getFieldValue('_T_STYLE')==='AnyNumber';
    if(b.state.pr==='modes'&&p.key==='M_MIN')return b.getFieldValue('_M_STYLE')!=='Between';
    if(b.state.pr==='announcement'&&p.key==='F_targets')return !b.getInputTargetBlock(p.key);
    if(b.state.pr==='mode'&&p.key==='F_weight'&&b.getFieldValue('_F_weight')==='1'){
      let owner=b.getParent();while(owner&&owner.state.ty.named!=='ModeSelection')owner=owner.getParent();
      try{if(api.fromBlock(owner).budget===null)return true;}catch(_){}
    }
    return null;
  }
  function scope(b) {
    let owner=b.getParent();while(owner&&!['Ability','Mode'].includes(owner.state.ty.named)&&!owner.getInput('F_announcement')&&!owner.getInput('O_announcement'))owner=owner.getParent();return owner;
  }
  function countOption(b) {
    if(b.state.ty.named==='TargetSpec')return b.state.simpleTargetCount?['Count expression…','COUNT_BLOCKS']:targetCount(api.fromBlock(b).count)?['Compact count','COUNT_SIMPLE']:null;
    if(b.state.pr==='modes')return b.state.simpleModeCount?['Count expressions…','COUNT_BLOCKS']:modeCount(api.fromBlock(b).count)?['Compact count','COUNT_SIMPLE']:null;
    return null;
  }
  function editCount(b,simple) {
    const key=b.state.ty.named==='TargetSpec'?'simpleTargetCount':'simpleModeCount';
    api.replaceView(b,{[key]:simple,shown:simple?(b.state.shown||[]).filter(k=>k!=='F_count'):[...new Set([...(b.state.shown||[]),'F_count'])]});
  }
  function targetSlots(parent,except=null) {
    const slots=new Set();
    for(let owner=parent;owner;owner=owner.getParent()){
      const announcements=[owner.state.ty.named==='Announcement'?owner:null,owner.getInputTargetBlock('F_announcement'),owner.getInputTargetBlock('O_announcement')].filter(Boolean);
      for(const a of announcements)for(let row=a.getInputTargetBlock('F_targets');row;row=row.getNextBlock())if(row!==except)slots.add(Number(row.getFieldValue('_F_slot')));
      if(owner.state.ty.named==='Ability')break;
    }
    return slots;
  }
  function addTarget(parent,patch={}) {
    if(parent.isInFlyout)return;
    const slots=targetSlots(parent);
    if(slots.size>=2048){api.notice('This demo allows up to 2,048 target groups.');return;}
    let slot=slots.size?Math.max(...slots)+1:0;if(slot>4294967295){slot=0;while(slots.has(slot))slot++;}
    const value=api.defaultValue({named:'TargetSpec'});Object.assign(value,api.clone(patch));value.slot=slot;
    api.remember();api.addToFlow(parent,api.parts(parent.state).find(p=>p.key==='F_targets'),value);return slot;
  }
  function addMode(parent) {
    if(parent.isInFlyout)return;
    const value=api.defaultValue({named:'Mode'});value.weight=1;
    api.remember();api.addToFlow(parent,api.parts(parent.state).find(p=>p.key==='F_options'),value);
  }
  function addModes(parent) {
    if(parent.isInFlyout)return;
    if(parent.state.directModes){parent.getInputTargetBlock('F_modes')?.setCollapsed(false);return;}
    const owner=scope(parent),ownerId=owner?.id;
    const value=api.fromBlock(parent),mode=api.defaultValue({named:'Mode'});mode.weight=1;
    value.modes=api.defaultValue({named:'ModeSelection'});value.modes.options=[api.clone(mode),api.clone(mode)];
    api.replaceView(parent,{directModes:true},value);
    // Existing effects become the first option. Move their blocks intact so
    // names and binding identities follow them into their new lexical scope.
    const freshOwner=ownerId&&api.workspace().getBlockById(ownerId),head=freshOwner?.getInputTargetBlock('F_body');
    if(head){const selection=api.workspace().getBlockById(parent.id).getInputTargetBlock('F_modes'),first=selection.getInputTargetBlock('F_options');head.previousConnection.disconnect();first.getInput('F_body').connection.connect(head.previousConnection);api.notice('Existing effects moved into Mode 1.');api.changed();}
  }
  function reveal(parent) {
    const announcement=parent.getInputTargetBlock('F_announcement');if(!announcement||parent.isInFlyout)return;
    api.remember();parent.state.shown=[...new Set([...(parent.state.shown||[]),'F_announcement'])];announcement.setCollapsed(false);api.applyPresentation();api.changed();
  }
  function removableRow(block) {
    const family=new Set(block.getDescendants(false));const next=block.getNextBlock();if(next)for(const b of next.getDescendants(false))family.delete(b);
    const ports=api.bindings().ports(block.getRootBlock()),keys=new Set(ports.filter(p=>p.definition&&family.has(p.block)).map(p=>api.bindings().metadata(p)?.key).filter(Boolean));
    if(ports.some(p=>!p.definition&&!family.has(p.block)&&(keys.has(api.bindings().metadata(p)?.key)||api.bindings().metadata(p)?.alternatives?.some(a=>keys.has(a.key)))))throw Error('An effect uses this target or mode result. Change those references before removing it.');
  }
  function removeRow(block) {
    if(block.isInFlyout)return;
    try{
      removableRow(block);
      if(block.state.pr==='mode'){
        let owner=block.getParent();while(owner&&owner.state.ty.named!=='ModeSelection')owner=owner.getParent();
        const value=api.fromBlock(owner);if(value.rules.repetition==='OnceEach'&&[value.count.minimum,value.count.maximum].some(n=>n.kind==='Constant'&&n.args>value.options.length-1))throw Error('Lower the mode count before removing this mode, or remove the whole modes section.');
      }
      api.remember();block.unplug(true);block.dispose(false);api.changed();
    }catch(e){api.notice(e.message);}
  }
  function clearAnnouncement(parent,key) {
    try{const child=parent.getInputTargetBlock(key);if(child)removableRow(child);const value=api.fromBlock(parent),values=api.childValues(parent.state.ty,value,parent.state);values[key]={targets:[],modes:null};api.replaceView(parent,{},api.assemble(parent.state,values));}catch(e){api.notice(e.message);}
  }
  function removeModes(parent) {
    try{const child=parent.getInputTargetBlock('F_modes');if(child)removableRow(child);const value=api.fromBlock(parent);value.modes=null;api.replaceView(parent,{directModes:false},value);}catch(e){api.notice(e.message);}
  }
  function moved(event) {
    if(event.type!==Blockly.Events.BLOCK_MOVE)return;
    const b=api.workspace().getBlockById(event.blockId);if(!b?.state?.freshTarget)return;
    let owner=b.getParent();while(owner&&owner.state.ty.named!=='Announcement')owner=owner.getParent();if(!owner)return;
    const used=targetSlots(owner,b);
    const id=Number(b.getFieldValue('_F_slot'));if(used.has(id)){let next=0;while(used.has(next))next++;b.setFieldValue(String(next),'_F_slot');}b.state.freshTarget=false;
  }
  function refresh(b) {
    if(b.state.pr==='mode'){
      let number=1,prev=b.getPreviousBlock();while(prev?.state.ty.named==='Mode'){number++;prev=prev.getPreviousBlock();}b.getField('MODE_TITLE')?.setValue('Mode '+number);
    }
    b.getField('ADD_MODES')?.setVisible(!b.state.directModes);
    b.getField('_T_NUMBER')?.setVisible(b.getFieldValue('_T_STYLE')!=='AnyNumber');
    const between=b.getFieldValue('_M_STYLE')==='Between';b.getField('_M_MIN')?.setVisible(between);b.getField('MODE_AND')?.setVisible(between);
    if(b.state.pr==='announcement')b.getField('CHOICES_TITLE')?.setValue(b.state.directModes?(b.getInputTargetBlock('F_targets')?'Targets & modes':'Modes'):'Targets');
  }
  return {state,parts,values,fields,targetCount,modeCount,countOption,editCount,managed,hidden,scope,addTarget,addMode,addModes,reveal,removeRow,removeModes,clearAnnouncement,refresh,moved};
}
