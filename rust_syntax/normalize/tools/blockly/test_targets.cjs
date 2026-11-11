/* Readable targeting/damage controls must preserve the exact Rust wire model. */
const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html';
 const url=/^https?:/.test(input)?input:pathToFileURL(require('node:path').resolve(input)).href;
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1580,height:1000}});page.setDefaultTimeout(30000);const errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 await page.goto(url,{timeout:120000});await page.waitForFunction(()=>window.cardBlocks?.valid());
 await page.evaluate(async()=>{const d=window.cardBlocks,e=d.payload.cards.find(c=>c.name==='Lightning Bolt');d.loadIR((await d.shard(e.shard))[e.index]);});
 await page.waitForFunction(()=>window.cardBlocks.valid());
 const initial=await page.evaluate(()=>{
  const d=window.cardBlocks,blocks=d.workspace.getAllBlocks(false),t=blocks.find(b=>b.state.ty.named==='TargetSpec'),damage=blocks.find(b=>b.state.op==='DealDamage');
  return {target:t.getField('TARGET_PRESET').getText(),collapsed:t.isCollapsed(),kind:t.getInput('F_kind').isVisible(),filter:t.getInput('F_filter').isVisible(),result:damage.getInput('RESULT').isVisible(),prevention:damage.getInput('O_preventable').isVisible(),checkbox:damage.getFieldValue('_O_preventable'),ir:d.generated()};
 });
 assert.equal(await page.evaluate(()=>window.cardBlocks.actionValue('DealDamage').args.operation.args.preventable),true);
 assert.equal(initial.target,'any target');assert.equal(initial.collapsed,false);assert.equal(initial.kind,false);assert.equal(initial.filter,false);assert.equal(initial.result,false);assert.equal(initial.prevention,false);assert.equal(initial.checkbox,'FALSE');
 const target=()=>page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0]);
 // Every target-kind search points to the same single editable library block.
 for(const query of ['target creature','target player','target spell','target opponent']){await page.locator('#block-search').fill(query);assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>b.state.ty.named==='TargetSpec').length),1);}
 await page.locator('#block-search').fill('');
 // One event starter offers every common event with When before its picker.
 await page.locator('.blocklyToolboxCategory').filter({has:page.getByText('Events',{exact:true})}).first().click();
 assert.deepEqual(await page.evaluate(()=>{const d=window.cardBlocks,rows=d.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>b.state.ty.named==='EventSpec'),b=rows[0];b.setFieldValue('5','_VALUE');return {count:rows.length,text:b.toString(),value:d.fromBlock(b)};}),{count:1,text:'When your upkeep begins',value:{Turn:{point:'UpkeepBegins',players:{kind:'One',args:{kind:'AbilityController'}}}}});
 await page.locator('#block-search').fill('upkeep');assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).filter(b=>b.state.ty.named==='EventSpec'&&b.state.pr==='literal').length),1);await page.locator('#block-search').fill('');
 // Select each choice through the actual dropdown menu.
 for(const [label,kind,filter] of [
  ['target creature',{Object:{kind:'Permanent',zones:['Battlefield']}},{kind:'CardType',args:'Creature'}],
  ['target planeswalker',{Object:{kind:'Permanent',zones:['Battlefield']}},{kind:'CardType',args:'Planeswalker'}],
  ['target player','Player',{kind:'Any'}],['any target','DamageRecipient',{kind:'Any'}],
 ]) {
  console.log('Selecting '+label);
  await page.evaluate(async()=>{await Blockly.renderManagement.finishQueuedRenders();window.cardBlocks.workspace.zoomToFit();});
  const point=await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='TargetSpec'),r=b.getField('TARGET_PRESET').getSvgRoot().getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};});
  await page.mouse.click(point.x,point.y);await page.getByRole('option',{name:label,exact:true}).click();
  await page.waitForFunction(kind=>JSON.stringify(window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0].kind)===JSON.stringify(kind)&&window.cardBlocks.valid(),kind);
  const v=await target();assert.deepEqual(v.kind,kind);assert.deepEqual(v.filter,filter);assert.equal(v.players,null);
 }
 // Target-slot changes are local to their ability, and undo restores the full operation.
 await page.evaluate(()=>{const d=window.cardBlocks,v=d.generated();v.faces[0].abilities.push(structuredClone(v.faces[0].abilities[0]));d.loadIR(v);});
 await page.waitForFunction(()=>window.cardBlocks.valid());
 const second=await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[1]);
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='TargetSpec').setFieldValue('1','TARGET_PRESET'));
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0].kind.Object&&window.cardBlocks.valid());
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[1]),second);
 await page.locator('#undo').click();
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0].kind==='DamageRecipient'&&window.cardBlocks.valid());
 await page.evaluate(value=>window.cardBlocks.loadIR(value),initial.ir);await page.waitForFunction(()=>window.cardBlocks.valid());
 // IR details does not expose unused result IDs, and display switches don't edit the model.
 await page.locator('#details').click();
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage').getInput('RESULT').isVisible()),false);
 assert.deepEqual(await page.evaluate(()=>window.cardBlocks.generated()),initial.ir);
 await page.locator('#details').click();
 // Add the exceptional damage modifier via ⋯, then remove it using its checkbox.
 const menu=await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage'),r=b.getField('MORE').getSvgRoot().getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};});
 await page.mouse.click(menu.x,menu.y);await page.getByRole('option',{name:"can't be prevented",exact:true}).click();
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable===false&&window.cardBlocks.valid());
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage').getFieldValue('_O_preventable')),'TRUE');
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.op==='DealDamage').setFieldValue('FALSE','_O_preventable'));
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.preventable===true&&window.cardBlocks.valid());
 console.log('Prevention toggle passed');
 // Complex targets stay custom. Domain changes retain non-domain details.
 await page.evaluate(()=>{const d=window.cardBlocks,v=d.generated(),t=v.faces[0].abilities[0].args.announcement.targets[0];t.filter={kind:'All',args:[{kind:'CardType',args:'Creature'},{kind:'Not',args:{kind:'Color',args:'Black'}}]};t.kind={Object:{kind:'Permanent',zones:['Battlefield']}};t.count={UpTo:{kind:'Constant',args:2}};v.faces[0].abilities[0].args.body.instructions[0].args.operation.args.recipients={kind:'Objects',args:{kind:'Targets',args:0}};d.loadIR(v);});
 await page.waitForFunction(()=>window.cardBlocks.valid());
 const custom=await target();
 assert.equal(await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='TargetSpec').getFieldValue('TARGET_PRESET')),'CUSTOM');
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='TargetSpec').setFieldValue('0','TARGET_PRESET'));
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0].kind==='DamageRecipient'&&window.cardBlocks.valid());
 assert.deepEqual((await target()).count,custom.count);
 // Saved workspaces predating the inverted checkbox migrate without flipping damage semantics.
 await page.evaluate(()=>{
  const d=window.cardBlocks,v=d.blank(true);v.faces[0].abilities[0].args.body.instructions[0].args.operation={kind:'DealDamage',args:{source:{kind:'Source'},recipients:{kind:'Players',args:{kind:'All'}},amount:{kind:'Constant',args:3},preventable:true}};d.loadIR(v,false);
  const saved=d.savedWorkspace();delete saved.presentation;
  function legacy(b){delete b.extraState.negativePrevention;for(const k of Object.keys(b.fields||{}))if(k.endsWith('preventable'))b.fields[k]=!(b.fields[k]===true||b.fields[k]==='TRUE');Object.values(b.inputs||{}).forEach(i=>{if(i.block)legacy(i.block);if(i.shadow)legacy(i.shadow);});if(b.next?.block)legacy(b.next.block);}
  saved.workspace.blocks.blocks.forEach(legacy);const before=d.encode(d.generated());d.restore(saved);if(d.encode(d.generated())!==before)throw Error('Migration changed prevention');
 });
 await page.waitForFunction(()=>window.cardBlocks.valid());
 assert.deepEqual(errors,[]);
 console.log(JSON.stringify({readableTargets:true,allFourChoices:true,invertedPrevention:true,hiddenResultIds:true,customFiltersPreserved:true,countsPreserved:true,legacyMigration:true,abilityScopePreserved:true,targetEditUndo:true,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(page)console.error(await page.evaluate(()=>({error:document.getElementById('validation-detail').textContent,ir:window.cardBlocks.generated()})));if(browser)await browser.close();process.exitCode=1;});
