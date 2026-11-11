const assert=require('node:assert/strict');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser,page;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html',url=/^https?:/.test(input)?input:pathToFileURL(require('node:path').resolve(input)).href;
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 page=await browser.newPage({viewport:{width:1580,height:1000},acceptDownloads:true});page.setDefaultTimeout(30000);
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(url,{timeout:120000});await page.waitForFunction(()=>window.cardBlocks?.valid());
 // Flyouts use their category colors and current visible block sizes.
 for(const [category,colour] of [['Abilities','#477d70'],['Choices & modes','#8762a8'],['Events','#508d88'],['Targets & selections','#5a7f9e'],['Values & conditions','#788544']]){
  await page.locator('.blocklyToolboxCategory').filter({has:page.getByText(category,{exact:true})}).first().click();await page.evaluate(()=>window.cardBlocks.applyPresentation());
  const view=await page.evaluate(()=>{const w=window.cardBlocks.workspace.getFlyout().getWorkspace();return {scale:w.scale,rows:w.getTopBlocks(true).map(b=>{const r=b.getSvgRoot().querySelector('.blocklyPath').getBoundingClientRect();return {colour:b.getColour(),top:r.top,bottom:r.bottom};})};});
  for(const row of view.rows)assert.equal(row.colour,colour,category);
  for(let i=1;i<view.rows.length;i++){const gap=view.rows[i].top-view.rows[i-1].bottom;assert.ok(gap>=-1&&gap<=100*view.scale,`${category}: unexpected gap ${gap}`);}
 }
 await page.locator('.blocklyToolboxCategory').filter({has:page.getByText('Flow',{exact:true})}).first().click();
 await page.evaluate(()=>window.cardBlocks.applyPresentation());
 assert.ok(await page.evaluate(()=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).every(b=>b.getColour()==='#9b7a42')));
 await page.locator('#starter').selectOption('spell');await page.locator('#new-card').click();await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities.length===1);
 await page.evaluate(async()=>{await Blockly.renderManagement.finishQueuedRenders();window.cardBlocks.workspace.setScale(.85);window.cardBlocks.workspace.scrollCenter();});
 async function dragFromMenu(category,source,target) {
  await page.locator('.blocklyToolboxCategory').filter({has:page.getByText(category,{exact:true})}).first().click();
  await page.evaluate(async()=>{await window.cardBlocks.applyPresentation();await Blockly.renderManagement.finishQueuedRenders();});
  if(category==='Effects'&&source!=='spell'){
   const point=await page.evaluate(source=>{const b=window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).find(b=>b.state.pr==='effect'&&b.getField('VARIANT')?.getOptions(false).some(([,name])=>name===source)),r=b.getField('VARIANT').getSvgRoot().getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2};},source);
   await page.mouse.click(point.x,point.y);await page.getByRole('option',{name:source==='GainLife'?'Gain Life':'Draw Cards',exact:true}).click();await page.waitForFunction(source=>window.cardBlocks.workspace.getFlyout().getWorkspace().getTopBlocks(false).some(b=>b.state.op===source),source);
  }
  const start=await page.evaluate(source=>{
   const d=window.cardBlocks,w=d.workspace.getFlyout().getWorkspace();
   const b=w.getTopBlocks(false).find(b=>source==='spell'?b.state.ty.named==='Ability'&&b.state.variant==='Spell':b.state.pr==='effect'&&b.state.op===source);
   if(!b)throw Error('Missing flyout block '+source);const box=b.getSvgRoot().getBoundingClientRect();return {x:box.x+Math.min(65,box.width/3),y:box.y+17};
  },source);
  await page.mouse.move(start.x,start.y);await page.mouse.down();await page.mouse.move(1050,450,{steps:25});
  await page.waitForTimeout(150);
  const offset=await page.evaluate(({source,target})=>{
   const w=window.cardBlocks.workspace;
   const b=w.getTopBlocks(false).find(b=>source==='spell'?b.state.ty.named==='Ability'&&b.state.variant==='Spell':b.state.pr==='effect'&&b.state.op===source);
   if(!b)throw Error('Dragging did not create a main-workspace block');
   let socket;
   if(target==='abilities')socket=w.getTopBlocks(false).find(b=>b.state.pr==='single').getInput('SECTION_resolve').connection;
   else if(target==='body')socket=w.getAllBlocks(false).find(b=>b.state.ty.named==='Ability'&&b.state.variant==='Spell').getInput('F_body').connection;
   else socket=w.getAllBlocks(false).find(b=>b.state.pr==='effect'&&b.state.op==='Draw').nextConnection;
   const point=c=>Blockly.utils.svgMath.wsToScreenCoordinates(w,new Blockly.utils.Coordinate(c.x,c.y));
   const box=b.getSvgRoot().getBoundingClientRect(),off=b.previousConnection.getOffsetInBlock();
   const a={x:box.x+off.x*w.scale,y:box.y+off.y*w.scale},z=point(socket);return {dx:z.x-a.x,dy:z.y-a.y};
  },{source,target});
  await page.mouse.move(1050+offset.dx,450+offset.dy,{steps:25});await page.waitForTimeout(100);await page.mouse.up();
  await page.waitForTimeout(250);
 }
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities.length===1&&window.cardBlocks.valid());
 await dragFromMenu('Effects','Draw','body');
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===1&&window.cardBlocks.valid());
 await dragFromMenu('Effects','GainLife','after-draw');
 await page.waitForFunction(()=>window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions.length===2&&window.cardBlocks.valid());
 // Inline editable parameters, with no number/player wrapper blocks.
 await page.evaluate(()=>window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.pr==='effect'&&b.state.op==='Draw').getInputTargetBlock('O_amount').setFieldValue('2','_VALUE'));
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.body.instructions[0].args.operation.args.amount.args===2);
 const value=await page.evaluate(()=>window.cardBlocks.generated());
 assert.deepEqual(value.faces[0].abilities[0].args.body.instructions.map(i=>i.args.operation.kind),['Draw','GainLife']);
 const presentation=await page.evaluate(()=>{
  const d=window.cardBlocks,w=d.workspace,root=w.getTopBlocks(false)[0],ability=w.getAllBlocks(false).find(b=>b.state.ty.named==='Ability');
  return {rootSchemaVisible:root.getInput('ROOT_schema').isVisible(),emptyAnnouncementVisible:ability.getInput('F_announcement').isVisible(),emptyBodyContainers:w.getAllBlocks(false).filter(b=>b.state.ty.named==='Program'||b.state.ty.vec?.named==='Instruction').length};
 });
 assert.equal(presentation.rootSchemaVisible,false);assert.equal(presentation.emptyAnnouncementVisible,false);assert.equal(presentation.emptyBodyContainers,0);
 // Advanced presentation is reversible and never edits the IR.
 const before=await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated()));
 await page.locator('#details').click();await page.locator('#details').click();
 assert.equal(await page.evaluate(()=>window.cardBlocks.encode(window.cardBlocks.generated())),before);
 // Optional targeting is created on demand rather than an empty announcement block.
 const menu=await page.evaluate(()=>{const b=window.cardBlocks.workspace.getAllBlocks(false).find(b=>b.state.ty.named==='Ability'&&b.state.variant==='Spell'),r=b.getField('MORE').getSvgRoot().getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};});
 await page.mouse.click(menu.x,menu.y);await page.getByText('Add a target',{exact:true}).click();
 await page.waitForFunction(()=>window.cardBlocks.valid()&&window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets.length===1);
 assert.equal(await page.evaluate(()=>window.cardBlocks.generated().faces[0].abilities[0].args.announcement.targets[0].count.Exactly.args),1);
 const downloadPromise=page.waitForEvent('download');await page.locator('#export').click();await downloadPromise;
 await page.evaluate(()=>{window.cardBlocks.workspace.getToolbox().clearSelection();window.cardBlocks.workspace.zoomToFit();});
 await page.screenshot({path:process.env.BLOCKLY_SCREENSHOT||'/tmp/card-blocks-authoring.png',fullPage:true});
 assert.deepEqual(errors,[]);
 console.log(JSON.stringify({realFlyoutDrag:true,stackedActions:true,inlineEditing:true,unusedFieldsHidden:true,noProgramWrappers:true,detailsLossless:true,onDemandTargeting:true,download:true,presentation,errors},null,2));
 await browser.close();
})().catch(async e=>{console.error(e);if(page){await page.screenshot({path:'/tmp/card-blocks-authoring-failure.png'});console.error(await page.evaluate(()=>({error:document.getElementById('validation-detail').textContent,model:window.cardBlocks.generated(),roots:window.cardBlocks.workspace.getTopBlocks(false).map(b=>({ty:b.state.ty,variant:b.state.variant,op:b.state.op,pos:b.getRelativeToSurfaceXY()}))})));}if(browser)await browser.close();process.exitCode=1;});
