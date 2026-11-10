/* Exercise locations, real removal controls, names, undo, and starter discovery. */
const assert=require('node:assert/strict'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 const target=process.argv[2]||'__build/card-blocks.html';
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1750,height:1150}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(/^https?:/.test(target)?target:pathToFileURL(require('node:path').resolve(target)).href,{timeout:120000});
 const valid=()=>page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:30000});await valid();
 async function load(name){await page.evaluate(async name=>{const d=window.cardBlocks;await d.loadCard(d.payload.cards.find(c=>c.name===name));},name);await valid();}
 const examples=[['Lightning Bolt',['resolve']],['Think Twice',['resolve','casting']],['Llanowar Elves',['battlefield']],['Anger',['battlefield','graveyard']],['Reassembling Skeleton',['battlefield','graveyard']],['Tarmogoyf',['battlefield','everywhere']],['Elder Deep-Fiend',['battlefield','casting']],['Lightning Storm',['resolve','stack']]];
 for(const [name,expected] of examples){await load(name);assert.deepEqual(await page.evaluate(()=>window.cardBlocks.workspace.getTopBlocks(false)[0].state.cardSections),expected,name);}
 // Imported abilities from different sections retain their original ordering and names.
 await load('Elder Deep-Fiend');const snapshot=await page.evaluate(()=>{const d=window.cardBlocks;return {ir:d.encode(d.generated()),doc:d.bindings.document(),saved:d.savedWorkspace()};});
 await page.evaluate(saved=>window.cardBlocks.restore(saved),snapshot.saved);await valid();assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),snapshot.ir);
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings),snapshot.doc.bindings);
 await page.locator('#starter').selectOption('spell');await page.locator('#new-card').click();await valid();
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.variant==='Spell').getField('VARIANT').getText()),'When this spell resolves');
 // Optional target sections can be added, removed with their own minus, and undone.
 async function menu(selector,label){await page.evaluate(selector=>{const b=selector==='spell'?window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.variant==='Spell'):window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op===selector||b.state.variant===selector);b.getField('MORE').showEditor_();},selector);await page.getByRole('option',{name:label,exact:true}).click();}
 async function clickField(selector,field){await page.evaluate(async()=>{window.cardBlocks.workspace.zoomToFit();await Blockly.renderManagement.finishQueuedRenders();});const pos=await page.evaluate(({selector,field})=>{const b=selector==='spell'?window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.variant==='Spell'):window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op===selector),box=b.getField(field).getSvgRoot().getBoundingClientRect();return {x:box.x+box.width/2,y:box.y+box.height/2};},{selector,field});await page.mouse.click(pos.x,pos.y);}
 const empty=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await menu('spell','Add a target');await valid();assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets.length),1);
 await clickField('spell','REMOVE_F_announcement');await valid();assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),empty);
 await page.locator('#undo').click();await valid();assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets.length),1);
 await page.locator('#redo').click();await valid();assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),empty);
 await load('Lightning Bolt');await menu('DealDamage',"can't be prevented");await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable===false);assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable),false);
 await clickField('DealDamage','REMOVE_O_preventable');await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable===true);assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable),true);
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage').getInput('O_preventable').isVisible()),false);
 await page.locator('#undo').click();await valid();assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable),false);
 // Create an activation in the graveyard and use a receipt twice in its body.
 await page.locator('#starter').selectOption('creature');await page.locator('#new-card').click();await valid();
 await page.evaluate(()=>{const d=window.cardBlocks,root=d.workspace.getTopBlocks(false)[0];d.addSection(root,'graveyard');const a=d.abilityValue('Activated');a.args.body.instructions=[d.actionValue('NoteObjects',{objects:{kind:'One',args:{kind:'Source'}}}),d.actionValue('NoteObjects',{objects:{kind:'Selected',args:0}})];a.args.body.instructions[0].args.result=0;a.args.body.instructions[1].args.result=1;d.addToFlow(root,d.parts(root.state).find(p=>p.abilitySection==='graveyard'),a);});await valid();
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.zones[0]),'Graveyard');
 await page.evaluate(async()=>{const d=window.cardBlocks,p=d.bindings.ports(d.workspace.getTopBlocks(false)[0]).find(p=>p.definition&&p.id===0);await d.bindings.rename(d.bindings.metadata(p).key,'remembered_card');});await valid();
 const named=await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.find(b=>b.name==='remembered_card'));
 // Deleting an earlier section must not transfer its names onto surviving abilities.
 await page.evaluate(()=>{const d=window.cardBlocks,r=d.workspace.getTopBlocks(false)[0];d.addToFlow(r,d.parts(r.state).find(p=>p.abilitySection==='battlefield'),d.abilityValue('Activated'));});await valid();
 await page.evaluate(()=>{const d=window.cardBlocks;d.removeSection(d.workspace.getTopBlocks(false)[0],'battlefield');});await valid();
 assert.equal(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.filter(b=>b.name==='remembered_card').length),1);

 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.variant==='Activated').setFieldValue('battlefield','LOCATION'));await valid();
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].kind==='Activated');await valid();
 assert.equal(await page.evaluate(()=>window.cardBlocks.bindings.document().bindings.find(b=>b.name==='remembered_card').explicit),named.explicit);
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[1].args.operation.args.objects.args),0);
 await page.locator('#undo').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].kind==='Scoped');
 // Multiple functioning zones are editable without changing the ability body.
 await menu('Activated','Edit functioning locations…');
 await page.locator('dialog[open] input').fill('Hand, Graveyard');await page.locator('dialog[open]').getByRole('button',{name:'OK',exact:true}).click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.zones.length===2);
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.zones),['Hand','Graveyard']);
 await page.locator('#undo').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.zones.length===1);
 // Empty sections are removable, and new known mechanics route to their home.
 await page.evaluate(()=>{const d=window.cardBlocks;d.removeSection(d.workspace.getTopBlocks(false)[0],'graveyard');});await valid();
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.workspace.getTopBlocks(false)[0].state.cardSections),['battlefield']);
 await page.locator('#block-search').fill('cycling');
 const cycling=await page.evaluate(()=>{const d=window.cardBlocks,w=d.workspace.getFlyout().getWorkspace(),b=w.getTopBlocks(false).find(b=>b.state.sourceScope?.includes('Hand'));if(!b)throw Error('No cycling starter');return d.fromBlock(b);});
 await page.locator('#block-search').fill('');
 await page.evaluate(value=>{const d=window.cardBlocks,r=d.workspace.getTopBlocks(false)[0];d.addToFlow(r,d.parts(r.state).find(p=>p.abilitySection==='battlefield'),value);},cycling);
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.workspace.getTopBlocks(false)[0].state.cardSections.includes('hand'));
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.zones),['Hand']);
 await page.locator('#undo').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities.length===0);
 // One plain keyword starter; no competing spell-ability starter to drag in.
 await page.locator('.blocklyToolboxCategory').filter({has:page.getByText('Abilities',{exact:true})}).first().click();
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>b.state.pr==='keyword').length),1);
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>b.state.variant==='Spell').length),0);
 await page.locator('#block-search').fill('haste');
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>b.state.pr==='keyword'&&b.state.keyword==='Haste').length),1);
 await page.locator('#block-search').fill('');
 await load('Anger');await page.evaluate(()=>window.cardBlocks.workspace.zoomToFit());
 await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/card-sections.png',fullPage:true});assert.deepEqual(errors,[]);
 console.log(JSON.stringify({cardLocations:true,automaticResolution:true,castTriggerSeparate:true,importOrderAndNames:true,removeOptionalTarget:true,removeUnpreventable:true,moveActivation:true,multipleLocations:true,receiptNamesRetained:true,undoRedo:true,removeSection:true,automaticCyclingLocation:true,singleKeywordStarter:true,errors},null,2));await browser.close();
})().catch(async e=>{console.error(e);if(page){console.error(await page.evaluate(()=>document.getElementById('validation-detail').textContent));await page.screenshot({path:'/tmp/card-sections-failure.png',fullPage:true});}if(browser)await browser.close();process.exitCode=1;});
