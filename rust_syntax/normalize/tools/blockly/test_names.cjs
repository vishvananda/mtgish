/* Actual named block editing, typed reference choices, persistence and copies. */
const assert=require('node:assert/strict'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1800,height:1200},acceptDownloads:true});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const target=process.argv[2]||'__build/card-blocks.html';await page.goto(/^https?:/.test(target)?target:pathToFileURL(require('node:path').resolve(target)).href,{timeout:120000});
 await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 // Unused results stay quiet until explicitly named through the normal menu.
 const unnamedIR=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 assert.equal(await page.evaluate(()=>!!window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage').getField('NAME:RESULT')),false);
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage').getField('MORE').showEditor_());
 await page.getByRole('option',{name:'Name result',exact:true}).click();
 await page.locator('input.blocklyHtmlInput').waitFor();await page.locator('input.blocklyHtmlInput').fill('dealt_damage');await page.locator('input.blocklyHtmlInput').press('Enter');
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.bindings.document().bindings.some(b=>b.kind==='Result'&&b.name==='dealt_damage'&&b.explicit));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),unnamedIR);
 await page.locator('#code-view').click();await page.locator('#dump-format').selectOption('human');
 const source='Card{name:"Named Bolt",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:victim,select:AnyTarget}],do:[hit=DealDamage{from:This,to:Targets(victim),amount:3}]}]}';
 await page.locator('#human-editor').fill(source);await page.locator('#apply-code').click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Named Bolt'&&document.getElementById('human-editor').value.includes('Targets(victim)'));
 const original=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='TargetSpec').getFieldValue('NAME:F_slot')),'victim');
 // Use the real text editor: typing must not close the widget or mutate the IR.
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='TargetSpec').getField('NAME:F_slot').showEditor_());
 await page.locator('input.blocklyHtmlInput').fill('defender');await page.waitForTimeout(250);
 assert.equal(await page.locator('input.blocklyHtmlInput').count(),1);
 await page.locator('input.blocklyHtmlInput').press('Enter');
 await page.waitForFunction(()=>window.cardBlocks.valid()&&document.getElementById('human-editor').value.includes('Targets(defender)'));
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),original);
 const saved=await page.evaluate(()=>window.cardBlocks.savedWorkspace());
 assert.ok(JSON.stringify(saved).includes('defender'));assert.equal(saved.presentation,6);
 await page.locator('#undo').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&document.getElementById('human-editor').value.includes('Targets(victim)'));
 await page.locator('#redo').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&document.getElementById('human-editor').value.includes('Targets(defender)'));
 await page.evaluate(saved=>window.cardBlocks.restore(saved),saved);await page.waitForFunction(()=>window.cardBlocks.valid());
 assert.match(await page.locator('#human-editor').inputValue(),/Targets\(defender\)/);
 await page.evaluate(saved=>{saved.model='previous-model';window.cardBlocks.restore(saved);},saved);await page.waitForFunction(()=>window.cardBlocks.valid());
 assert.match(await page.locator('#human-editor').inputValue(),/Targets\(defender\)/);
 await page.reload();await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});await page.locator('#code-view').click();await page.locator('#dump-format').selectOption('human');
 await page.waitForFunction(()=>document.getElementById('human-editor').value.includes('Targets(defender)'));
 // A name collision is rejected without changing either names or engine IDs.
 const two='Card{name:"Two targets",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:first,select:Creature},Target{name:second,select:Creature}],do:[NoteObjects{objects:Targets(first)}]}]}';
 await page.locator('#human-editor').fill(two);await page.locator('#apply-code').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Two targets'&&document.getElementById('human-editor').value.includes('name: second'));
 const choices=await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>!p.definition&&p.kind==='Target');return d.bindings.options(p);});
 assert.deepEqual(choices.map(([name])=>name).sort(),['first','second']);
 const rejected=await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&p.kind==='Target');try{await d.bindings.rename(d.bindings.metadata(p).key,'second');return false;}catch(_){return true;}});assert.equal(rejected,true);
 assert.match(await page.locator('#human-editor').inputValue(),/Targets\(first\)/);
 await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>!p.definition&&p.kind==='Target');d.selectors.expand(p.block);await Blockly.renderManagement.finishQueuedRenders();d.workspace.centerOnBlock(p.block.id);});
 await page.waitForFunction(()=>window.cardBlocks.valid());
 await page.evaluate(()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>!p.definition&&p.kind==='Target');p.block.getField('REF:'+p.key).showEditor_();});
 await page.getByRole('option',{name:'second',exact:true}).click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&document.getElementById('human-editor').value.includes('Targets(second)'));
 // Player/object loops share numeric labels but have independent typed names.
 const roles='Card{name:"Roles",properties:{card_types:[Instant]},abilities:[Spell{do:[ForEachPlayer{players:All,binding:b0,starting_with:None,in_chosen_direction:false,do:[ForEachObject{objects:One(This),binding:b0,do:[DrawCards{players:One(Bound(b0)),amount:1},NoteObjects{objects:One(Bound(b0))}]}]},picked=WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(picked)}}]}]}';
 await page.locator('#human-editor').fill(roles);await page.locator('#apply-code').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Roles');
 const roleIR=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 for(const [kind,variant,name] of [['Loop','ForEachPlayer','each_player'],['Loop','ForEachObject','each_object'],['Selection',null,'chosen_objects'],['Result',null,'remembered']]) {
  await page.evaluate(async({kind,variant,name})=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&p.kind===kind&&(variant?p.block.state.variant===variant:d.bindings.metadata(p).name==='picked'));await d.bindings.rename(d.bindings.metadata(p).key,name);},{kind,variant,name});
  await page.waitForFunction(()=>window.cardBlocks.valid());
 }
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),roleIR);
 const roleCode=await page.locator('#human-editor').inputValue();assert.match(roleCode,/Bound\(each_player\)/);assert.match(roleCode,/Bound\(each_object\)/);assert.match(roleCode,/Prepared\(chosen_objects\)/);assert.match(roleCode,/remembered =/);
 const roleMenus=await page.evaluate(async()=>{const d=window.cardBlocks,ports=d.bindings.ports(d.workspace.getTopBlocks(false)[0]);return Promise.all(ports.filter(p=>!p.definition&&p.kind==='Loop').map(async p=>(await d.bindings.options(p)).map(([name])=>name)));});
 assert.deepEqual(roleMenus.map(names=>names.length),[1,1]);
 // Equal numeric IDs in independent abilities must not silently capture a moved reference.
 const scopes='Card{name:"Scoped targets",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:t0,select:Creature}],do:[NoteObjects{objects:Targets(t0)}]},Spell{targets:[Target{name:t0,select:Creature}],do:[NoteObjects{objects:Targets(t0)}]}]}';
 await page.locator('#human-editor').fill(scopes);await page.locator('#apply-code').click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Scoped targets');
 await page.evaluate(()=>{const d=window.cardBlocks,abilities=d.workspace.getAllBlocks(false).filter(b=>b.state.ty.named==='Ability');const moved=abilities[0].getInputTargetBlock('F_body');moved.unplug(false);abilities[1].getInputTargetBlock('F_body').nextConnection.connect(moved.previousConnection);});
 await page.waitForFunction(()=>!window.cardBlocks.valid()&&document.getElementById('validation-detail').textContent.includes('scope'));
 assert.equal(await page.locator('#export').isDisabled(),true);
 assert.equal(await page.evaluate(async()=>{const d=window.cardBlocks;return (await d.validateTexts([d.encode(d.generated())]))[0].valid;}),true);
 const recovery=await page.evaluate(async()=>{const d=window.cardBlocks,ports=d.bindings.ports(d.workspace.getTopBlocks(false)[0]),p=ports.find(p=>!p.definition&&p.kind==='Target'&&p.path.includes('/abilities/1/')&&ports.filter(q=>q.kind==='Target'&&d.bindings.metadata(q)?.key===d.bindings.metadata(p)?.key).some(q=>q.path.includes('/abilities/0/')));if(!p)throw Error('No moved reference');const choices=await d.bindings.options(p);if(choices.length!==1)throw Error('Wrong scope choices '+JSON.stringify(choices));await d.bindings.choose(p,choices[0][1]);return choices.length;});
 assert.equal(recovery,1);await page.waitForFunction(()=>window.cardBlocks.valid());
 // Actual copied blocks get fresh definitions and maintain their internal links.
 const copying='Card{name:"Copies",properties:{card_types:[Instant]},abilities:[Spell{do:[chosen_cards=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen_cards)}]}]}';
 await page.locator('#human-editor').fill(copying);await page.locator('#apply-code').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Copies'&&document.getElementById('human-editor').value.includes('chosen_cards'));
 const clip=await page.evaluate(()=>{const d=window.cardBlocks,ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability');return Blockly.serialization.blocks.save(ability.getInputTargetBlock('F_body'),{addNextBlocks:true});});
 for(const external of [false,true]) {
  await page.evaluate(({clip,external})=>{
   const d=window.cardBlocks,ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability');
   function foreign(node){for(const m of Object.values(node.extraState?.bindings||{}))m.key='foreign:'+m.key;for(const input of Object.values(node.inputs||{})){if(input.block)foreign(input.block);if(input.shadow)foreign(input.shadow);}if(node.next?.block)foreign(node.next.block);}
   if(external)foreign(clip);const copy=Blockly.serialization.blocks.append(clip,d.workspace);let tail=ability.getInputTargetBlock('F_body');while(tail.getNextBlock())tail=tail.getNextBlock();tail.nextConnection.connect(copy.previousConnection);
  },{clip,external});
  await page.waitForFunction(count=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===count,external?6:4);
 }
 // Cutting and pasting the same statement group preserves its authoring identity.
 const cut=await page.evaluate(()=>{
  const d=window.cardBlocks,ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability'),head=ability.getInputTargetBlock('F_body');
  const state=Blockly.serialization.blocks.save(head,{addNextBlocks:true}),doc=d.bindings.document();head.dispose(false);return {state,names:doc.bindings.map(b=>b.name)};
 });
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===0);
 await page.evaluate(cut=>{const d=window.cardBlocks,ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability'),head=Blockly.serialization.blocks.append(cut.state,d.workspace);ability.getInput('F_body').connection.connect(head.previousConnection);},cut);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===6);
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.map(b=>b.name)),cut.names);
 const copied=await page.locator('#human-editor').inputValue();assert.match(copied,/chosen_cards_copy/);assert.match(copied,/Selected\(chosen_cards_copy\)/);
 const download=page.waitForEvent('download');await page.locator('#export-code').click();const file=await download;const exported=require('node:fs').readFileSync(await file.path(),'utf8');assert.match(exported,/chosen_cards_copy/);
 // Copies of a closure retain captures from outside the copied group.
 const capture='Card{name:"Captured names",properties:{card_types:[Instant]},abilities:[Spell{do:[saved=NoteObjects{objects:One(This)},CreateDelayedTrigger{event:Turn{point:EndStepBegins,players:All},start:Immediately,expires:None,intervening_if:None,do:[captured=NoteObjects{objects:Selected(saved)},NoteObjects{objects:Selected(captured)}]}]}]}';
 await page.locator('#human-editor').fill(capture);await page.locator('#apply-code').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Captured names');
 await page.evaluate(()=>{const d=window.cardBlocks,ability=d.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability'),source=ability.getInputTargetBlock('F_body').getNextBlock(),copy=Blockly.serialization.blocks.append(Blockly.serialization.blocks.save(source,{addNextBlocks:false}),d.workspace);source.nextConnection.connect(copy.previousConnection);});
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===3);
 const captures=await page.locator('#human-editor').inputValue();assert.equal((captures.match(/Selected\(saved\)/g)||[]).length,2);assert.match(captures,/Selected\(captured_copy\)/);
 await page.locator('#human-editor').fill(source.replace(/victim/g,'defender').replace('hit=',''));await page.locator('#apply-code').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].name==='Named Bolt');
 await page.evaluate(async()=>{await Blockly.renderManagement.finishQueuedRenders();window.cardBlocks.workspace.zoomToFit();});
 assert.deepEqual(errors,[]);await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/card-names.png',fullPage:true});
 console.log(JSON.stringify({typedNameFields:true,optionalResultMenu:true,realRename:true,idsUnchanged:true,undoRedo:true,saveReload:true,modelUpgrade:true,typedReferenceMenu:true,collisionRejected:true,copyDefinitions:true,foreignClipboardDefinitions:true,cutPastePreservesIdentity:true,externalCapturesPreserved:true,loopAndSelectionNames:true,crossScopeMoveRejected:true,explicitScopeRecovery:true,exportNames:true,errors},null,2));await browser.close();
})().catch(async e=>{console.error(e);if(page){console.error(await page.locator('#validation-detail').textContent().catch(()=>''));await page.screenshot({path:'/tmp/card-names-failure.png'}).catch(()=>{});}if(browser)await browser.close();process.exitCode=1;});
