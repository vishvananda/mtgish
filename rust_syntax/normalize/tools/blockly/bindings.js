/* Authoring identities travel with block ports; wire paths are rebuilt from the
 * current tree. Rust owns scope, reference types, and the final name check. */
'use strict';
function createBindingEditor(api) {
  const kinds={Slot:'Target',ResultId:'Result',Binding:'Loop',SelectionId:'Selection'};
  const clone=v=>structuredClone(v), ptr=(p,k)=>p+'/'+String(k).replace(/~/g,'~0').replace(/\//g,'~1');
  const fresh=()=>Blockly.utils.idGenerator.genUid();
  let accepted=null, acceptedIR=null, groupKeys=[], candidates=new Map(), knownKeys=new Set(), pendingEditor=null;
  function owner(s) {return s.pr==='effect'?'Operation::'+s.op:s.pr==='keyword'?'Keyword::'+s.keyword:s.variant?s.ty.named+'::'+s.variant:s.ty.named;}
  function definition(b,p) {
    const s=b.state,o=owner(s),key=p.key.replace(/^(F_|O_|K_)/,'');
    return s.pr==='effect'&&p.key==='RESULT' || o==='Instruction::Effect'&&key==='result'
      || o==='TargetSpec'&&key==='slot' || key==='binding'&&['Instruction::ForEachPlayer','Instruction::ForEachObject','Condition::ExistsPlayer','PlayerSet::Matching','SelectionStep'].includes(o);
  }
  function typedIds(t,value,path,emit) {
    if(kinds[t.named]){emit(path,kinds[t.named],value);return;}
    if(t.vec){value.forEach((v,i)=>typedIds(t.vec,v,ptr(path,i),emit));return;}
    if(t.option){if(value!==null)typedIds(t.option,value,path,emit);return;}
    const d=api.descriptor(t),v=api.variantFor(t,value),f=t.tuple?{kind:'tuple',items:t.tuple}:v?v.fields:d?.fields;
    if(!f)return;
    const base=v?ptr(path,d.adjacent?'args':v.wire):path,args=v?(d.adjacent?value.args:value[v.wire]):value;
    if(f.kind==='record')for(const field of f.fields)typedIds(field.ty,args[field.name],ptr(base,field.name),emit);
    if(f.kind==='tuple')f.items.forEach((ty,i)=>typedIds(ty,f.items.length===1?args:args[i],f.items.length===1?base:ptr(base,i),emit));
  }
  function setAt(value,path,id) {
    const keys=path.slice(1).split('/').map(k=>k.replace(/~1/g,'/').replace(/~0/g,'~'));
    let v=value;for(const key of keys.slice(0,-1))v=v[key];v[keys.at(-1)]=id;
  }
  function localPorts(b) {
    const out=[];
    for(const p of api.parts(b.state)) {
      const kind=kinds[p.ty.named];
      if(kind)out.push({block:b,part:p,key:p.key,kind,id:Number(b.getFieldValue('_'+p.key)),definition:definition(b,p),set:id=>b.setFieldValue(String(id),'_'+p.key)});
      else if(p.inline==='choice') {
        const index=Number(b.getFieldValue('_'+p.key)),value=api.pickerValue(b,p,index);
        typedIds(p.ty,value,'',(suffix,kind,id)=>out.push({block:b,part:p,key:p.key+'#'+suffix,suffix,kind,id,definition:false,set:id=>{
          const value=clone(api.pickerValue(b,p,Number(b.getFieldValue('_'+p.key))));setAt(value,suffix,id);
          (b.state.picks??={})[p.key]={index:Number(b.getFieldValue('_'+p.key)),value};
        }}));
      }
    }
    return out;
  }
  function partPath(s,p,path) {
    if(s.pr==='literal'||api.scalar(s.ty))return path;
    if(s.pr==='single')return p.key==='ROOT_schema'?ptr(path,'schema_version'):p.key==='ROOT_layout'?ptr(path,'layout'):p.key==='ROOT_data'?ptr(path,'data'):ptr(ptr(ptr(path,'faces'),0),p.key.slice(2));
    let base=path,prefix='F_',f;
    if(s.pr==='effect') {
      if(p.key==='RESULT')return ptr(ptr(path,'args'),'result');
      base=ptr(ptr(path,'args'),'operation');const d=api.types.Operation;f=api.viewVariant(s).fields;prefix='O_';base=ptr(base,d.adjacent?'args':s.op);
    }else if(s.pr==='keyword') {const d=api.types.Keyword;f=api.viewVariant(s).fields;prefix='K_';base=ptr(ptr(path,'args'),d.adjacent?'args':s.keyword);}
    else if(s.ty.vec)return ptr(path,Number(p.key.slice(5)));
    else if(s.ty.option)return path;
    else {const d=api.descriptor(s.ty),v=api.viewVariant(s);f=s.ty.tuple?{kind:'tuple',items:s.ty.tuple}:v?v.fields:d.fields;if(v)base=ptr(base,d.adjacent?'args':v.wire);}
    if(f.kind==='record')return ptr(base,p.key.slice(prefix.length));
    return f.items.length===1?base:ptr(base,Number(p.key.replace('ARG_','')));
  }
  function ports(root,value=api.fromBlock(root)) {
    const result=[];
    function chain(head,values,path) {let i=0;for(let b=head;b;b=b.getNextBlock()){visit(b,values[i],ptr(path,i));++i;}return i;}
    function visit(b,value,path) {
      const s=b.state,values=api.childValues(s.ty,value,s),locals=localPorts(b);let offset=0;
      for(const p of api.parts(s)) {
        let at=partPath(s,p,path);
        if(s.ty.vec&&['Ability','Instruction'].includes(s.ty.vec.named)) {
          let i=0;for(let child=b.getInputTargetBlock(p.key);child;child=child.getNextBlock())visit(child,value[offset+i],ptr(path,offset+i++));offset+=i;continue;
        }
        for(const port of locals.filter(port=>port.part.key===p.key))result.push({...port,path:at+(port.suffix||'')});
        if(p.inline||api.scalar(p.ty))continue;
        const child=b.getInputTargetBlock(p.key);if(!child&&!p.flow)throw Error('Fill the missing '+p.label+' block before naming references.');
        if(p.flow)chain(child,p.flow==='program'?values[p.key].instructions:values[p.key],p.flow==='program'?ptr(at,'instructions'):at);
        else visit(child,values[p.key],at);
      }
    }
    visit(root,value,'');return result;
  }
  const metadata=p=>p.block.state.bindings?.[p.key];
  function assign(p,m) {(p.block.state.bindings??={})[p.key]={...m,id:p.id};}
  function root() {const roots=api.workspace().getTopBlocks(false);if(roots.length!==1)throw Error('Keep one connected card before editing references.');return roots[0];}
  function currentPorts() {return ports(root());}
  function transfer(before,after) {
    const old=new Map(ports(before).map(p=>[p.path,p]));
    for(const p of ports(after)){const prior=old.get(p.path),m=prior&&metadata(prior);if(m&&prior.kind===p.kind&&(p.definition||prior.id===p.id))assign(p,m);}
  }
  function attach(root,doc) {
    const byPath=new Map(ports(root).map(p=>[p.path,p])), keys=doc.bindings.map(()=>fresh());
    for(const use of doc.uses){const p=byPath.get(use.path),b=doc.bindings[use.binding];if(!p||p.kind!==b.kind)throw Error('The block codec has no binding port for '+use.path);assign(p,{...b,key:keys[use.binding]});}
  }
  function merge(doc,list) {
    const byPath=new Map(list.map(p=>[p.path,p])), grouped=doc.bindings.map(()=>[]), seen=new Map(),keys=[],errors=[];
    for(const use of doc.uses) {
      const p=byPath.get(use.path);if(!p||p.kind!==doc.bindings[use.binding].kind)throw Error('Missing typed binding port: '+use.path);
      const prior=metadata(p),m=prior?.kind===p.kind&&(p.definition||prior.id===p.id)?prior:null;
      grouped[use.binding].push({port:p,meta:m});
    }
    const used=new Set(grouped.flat().map(x=>x.meta?.name).filter(Boolean));
    doc.bindings.forEach((b,i)=>{
      const entries=grouped[i],defs=entries.filter(x=>x.port.definition),anchors=(defs.length?defs:entries).map(x=>x.meta).filter(Boolean);
      const identities=[...new Set(anchors.map(m=>m.key))];
      if(identities.length>1)errors.push('Two declarations have conflicting binding identities.');
      const key=identities[0]||fresh();
      if(seen.has(key)&&seen.get(key)!==i)errors.push('A binding identity occurs in two scopes.');seen.set(key,i);keys.push(key);
      const retained=entries.map(x=>x.meta).filter(m=>m?.key===key);
      if(entries.some(x=>x.meta&&x.meta.key!==key))errors.push('A reference moved into a different binding scope. Choose its intended name again.');
      if(retained.length){if(retained.some(m=>m.name!==retained[0].name))errors.push('A binding has conflicting saved names.');b.name=retained[0].name;b.explicit=retained.some(m=>m.explicit);}
      else {const base=b.kind==='Loop'?(defs.some(x=>/ForEachPlayer|ExistsPlayer|Matching/.test(owner(x.port.block.state)))?'player':'item'):b.kind.toLowerCase();let name=base,n=2;while(used.has(name))name=base+'_'+n++;b.name=name;used.add(name);}
    });
    return {doc,keys,byPath,errors};
  }
  async function check(ir,revision) {
    const described=await api.request('describeBindings',ir);if(!described.valid)throw Error(described.error);
    if(revision!==api.revision())return null;
    const state=merge(api.parse(described.document),currentPorts()),formatted=await api.request('formatDocument',api.encode(state.doc));
    if(revision!==api.revision())return null;
    if(!formatted.valid)throw Error(formatted.error);
    // Retain a checked native context for repairing a moved reference. Its old
    // authoring identity is never overwritten implicitly, and export stays off.
    accepted=state.doc;acceptedIR=ir;groupKeys=state.keys;for(const key of groupKeys)knownKeys.add(key);candidates.clear();
    for(const use of state.doc.uses){const p=state.byPath.get(use.path),m=metadata(p),key=state.keys[use.binding];if(!m||m.kind!==p.kind||!p.definition&&m.id!==p.id||m.key===key)assign(p,{...state.doc.bindings[use.binding],key});}
    refresh();
    if(state.errors.length)throw Error(state.errors[0]);
    if(pendingEditor){const p=currentPorts().find(p=>p.definition&&metadata(p)?.key===pendingEditor);pendingEditor=null;if(p)requestAnimationFrame(()=>p.block.getField('NAME:'+p.key)?.showEditor_());}
    return {document:accepted,code:formatted.code};
  }
  function documentNow(strict=true) {
    if(!accepted||api.encode(api.generated())!==acceptedIR)throw Error('Wait for the latest card check before editing names.');
    const state=merge(clone(accepted),currentPorts());if(strict&&state.errors.length)throw Error(state.errors[0]);groupKeys=state.keys;return state.doc;
  }
  async function rename(key,name) {
    const start=api.revision(),doc=documentNow(false),i=groupKeys.indexOf(key);if(i<0)throw Error('This binding no longer exists.');
    doc.bindings[i].name=name;doc.bindings[i].explicit=true;
    const checked=await api.request('formatDocument',api.encode(doc));if(start!==api.revision())throw Error('The card changed while checking the name. Try again.');if(!checked.valid)throw Error(checked.error);
    api.remember();for(const p of currentPorts())if(metadata(p)?.key===key)assign(p,{...metadata(p),name,explicit:true});refresh();api.changed();return true;
  }
  async function options(port) {
    const start=api.revision(),doc=documentNow(false),out=[];
    for(let i=0;i<doc.bindings.length;i++) {
      const b=doc.bindings[i];if(b.kind!==port.kind)continue;
      const candidate=clone(doc),use=candidate.uses.find(u=>u.path===port.path);if(!use)continue;
      const definition=candidate.uses.find(u=>u.binding===i),id=definition&&getAt(candidate.card,definition.path);if(id===undefined)continue;
      setAt(candidate.card,port.path,id);use.binding=i;
      const checked=await api.request('formatDocument',api.encode(candidate));if(start!==api.revision())return [];
      if(checked.valid)out.push([b.name,groupKeys[i]]);
    }
    return out;
  }
  function getAt(value,path) {return path.slice(1).split('/').reduce((v,k)=>v[k.replace(/~1/g,'/').replace(/~0/g,'~')],value);}
  async function choose(port,key) {
    const list=await options(port);if(!list.some(([,k])=>k===key))throw Error('That name is not available in this scope or reference type.');
    const i=groupKeys.indexOf(key),doc=documentNow(false),site=doc.uses.find(u=>u.binding===i);if(!site)throw Error('Missing binding.');
    const id=getAt(doc.card,site.path),b=doc.bindings[i];api.remember();port.set(id);port.id=id;assign(port,{...b,key});api.changed();
  }
  function refresh() {
    let list;try{list=currentPorts();}catch(_){return;}
    Blockly.Events.disable();try{
    const used=new Map();for(const p of list){const m=metadata(p);if(m)used.set(m.key,(used.get(m.key)||0)+1);}
    for(const b of api.workspace().getAllBlocks(false))for(const input of [...b.inputList])if(input.name.startsWith('SYMBOL:'))b.removeInput(input.name);
    for(const p of list) {
      const m=metadata(p);if(!m)continue;
      if(p.definition&&p.kind==='Result'&&!m.explicit&&(used.get(m.key)||0)<2)continue;
      const input=p.block.appendDummyInput('SYMBOL:'+p.key);
      if(p.definition) {
        const field=new Blockly.FieldTextInput(m.name);
        // Keep the editor open while typing; validate the rename when committed.
        field.onFinishEditing_=value=>{const old=metadata(p)?.name;Blockly.Events.disable();try{field.setValue(old);}finally{Blockly.Events.enable();}if(value!==old)setTimeout(()=>rename(m.key,value).catch(e=>api.notice(e.message)),0);};
        field.SERIALIZABLE=false;input.appendField(p.kind==='Result'?'save as':'named').appendField(field,'NAME:'+p.key);
      }else {
        const cacheKey=api.revision()+':'+p.path,field=new Blockly.FieldDropdown(()=>candidates.get(cacheKey)||[[metadata(p)?.name||m.name,metadata(p)?.key||m.key]],value=>{
          if(value===metadata(p)?.key)return value;setTimeout(()=>choose(p,value).catch(e=>api.notice(e.message)),0);return null;
        });
        const open=field.showEditor_;field.showEditor_=function(event){
          if(candidates.has(cacheKey)){open.call(this,event);return;}
          options(p).then(values=>{if(!values.length)return;candidates.set(cacheKey,values);if(this.getSourceBlock()?.workspace)open.call(this,event);}).catch(e=>api.notice(e.message));
        };
        field.SERIALIZABLE=false;input.appendField(field,'REF:'+p.key);
      }
      if(p.block.rendered)p.block.render();
    }
    }finally{Blockly.Events.enable();}
  }
  function showNumeric(b,p) {return !kinds[p.ty.named];}
  function promote(b) {
    const p=localPorts(b).find(p=>p.definition&&p.kind==='Result'),m=p&&metadata(p);if(!m)return;
    let name=m.name,n=2;const names=new Set(accepted?.bindings.filter((_,i)=>groupKeys[i]!==m.key).map(b=>b.name));while(names.has(name))name='result_'+n++;
    rename(m.key,name).then(()=>{pendingEditor=m.key;}).catch(e=>api.notice(e.message));
  }
  function canPromote(b) {return localPorts(b).some(p=>p.definition&&p.kind==='Result'&&metadata(p)&&!metadata(p).explicit);}
  function created(ids) {
    const selected=new Set(ids),all=api.workspace().getAllBlocks(false).flatMap(localPorts),outside=new Set(all.filter(p=>p.definition&&!selected.has(p.block.id)).map(p=>metadata(p)?.key).filter(Boolean));
    const used={},names=new Set(all.map(p=>metadata(p)?.name).filter(Boolean)),copies=new Map();for(const p of all)(used[p.kind]??=new Set()).add(p.id);
    for(const p of all)if(selected.has(p.block.id)&&p.definition) {
      const m=metadata(p);if(!m||copies.has(m.key)||knownKeys.has(m.key)&&!outside.has(m.key))continue;
      let id=0;while(used[p.kind].has(id))++id;used[p.kind].add(id);let name=m.name+'_copy',n=2;while(names.has(name))name=m.name+'_copy_'+n++;names.add(name);
      copies.set(m.key,{...m,key:fresh(),name,id});
    }
    for(const p of all)if(selected.has(p.block.id)){const m=metadata(p),copy=m&&copies.get(m.key);if(copy){p.set(copy.id);p.id=copy.id;assign(p,copy);}}
  }
  return {ports,localPorts,attach,transfer,check,document:documentNow,rename,choose,options,refresh,showNumeric,promote,canPromote,created,metadata,reset(){accepted=null;acceptedIR=null;groupKeys=[];candidates.clear();knownKeys.clear();pendingEditor=null;}};
}
