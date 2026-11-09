/* Test the actual Wasm authoring-document bridge, independent of editor wiring. */
const assert=require('node:assert/strict'),{pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
let browser;
(async()=>{
 const input=process.argv[2]||'__build/card-blocks.html',url=/^https?:/.test(input)?input:pathToFileURL(require('node:path').resolve(input)).href;
 browser=await chromium.launch({headless:true,...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE?{executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}:{})});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(url,{timeout:120000});await page.waitForFunction(()=>window.cardBlocks?.valid(),null,{timeout:120000});
 const result=await page.evaluate(async()=>{
  const d=window.cardBlocks;
  const source='Card{name:"Named Bolt",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:victim,select:AnyTarget}],do:[hit=DealDamage{from:This,to:Targets(victim),amount:3}]}]}';
  const parsed=await d.parseCode(source);if(!parsed.valid)throw Error(parsed.error);
  const doc=d.parseExact(parsed.document),before=d.encode(doc.card);
  const formatted=await d.formatDocument(d.encode(doc));if(!formatted.valid)throw Error(formatted.error);
  const binding=doc.bindings.find(b=>b.kind==='Target'&&b.name==='victim');binding.name='defender';
  const renamed=await d.formatDocument(d.encode(doc));if(!renamed.valid)throw Error(renamed.error);
  const after=d.encode(doc.card);
  const described=await d.describeBindings(parsed.ir);if(!described.valid)throw Error(described.error);
  const unknown=d.parseExact(parsed.document);unknown.card.unexpected=true;const rejected=await d.formatDocument(d.encode(unknown));
  const scopes=await d.parseCode('Card{name:"Scopes",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:t0,select:Creature}],do:[NoteObjects{objects:Targets(t0)}]},Spell{targets:[Target{name:t0,select:Creature}],do:[NoteObjects{objects:Targets(t0)}]}]}');
  if(!scopes.valid)throw Error(scopes.error);const wrong=d.parseExact(scopes.document),targets=wrong.bindings.map((b,i)=>[b,i]).filter(([b])=>b.kind==='Target').map(([,i])=>i);
  wrong.uses.find(u=>u.binding===targets[1]&&u.path.endsWith('/objects/args')).binding=targets[0];
  const cross=await d.formatDocument(d.encode(wrong));
  const preparation=await d.parseCode('Card{name:"Selection captures",properties:{card_types:[Instant]},abilities:[Spell{do:[WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:WithSelections{steps:[{binding:picked,chooser:Players(You),objects:Prepared(picked),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(picked)}}}]}]}');
  if(!preparation.valid)throw Error(preparation.error);
  const prep=d.parseExact(preparation.document),selections=prep.bindings.filter(b=>b.kind==='Selection');
  if(selections.length!==2||selections[0].scope===selections[1].scope)throw Error('Selection identities lost their scopes');
  selections[0].name='outer_cards';selections[1].name='inner_cards';
  const captures=await d.formatDocument(d.encode(prep));if(!captures.valid)throw Error(captures.error);
  for(const name of ['Explosion of Riches',"Warrior's Blades"]) {
   const entry=d.payload.cards.find(c=>c.name===name);
   const card=(await d.shard(entry.shard))[entry.index],cohort=await d.describeBindings(d.encode(card));if(!cohort.valid)throw Error(cohort.error);
   const scoped=d.parseExact(cohort.document);scoped.bindings.forEach((b,i)=>{b.name='named_'+i;b.explicit=true;});
   const renamed=await d.formatDocument(d.encode(scoped));if(!renamed.valid)throw Error(name+': '+renamed.error);
  }
  return {ir:parsed.ir,before,after,formatted:formatted.code,renamed:renamed.code,described:described.valid,rejected,cross,captures:captures.code};
 });
 assert.ok(result.formatted.includes('hit = DealDamage'));assert.ok(result.formatted.includes('Targets(victim)'));
 assert.ok(result.renamed.includes('name: defender'));assert.ok(result.renamed.includes('Targets(defender)'));assert.ok(!result.renamed.includes('victim'));
 assert.equal(result.before,result.after);assert.ok(!result.ir.includes('victim'));assert.equal(result.described,true);
 assert.equal(result.rejected.valid,false);assert.match(result.rejected.error,/unknown/);
 assert.equal(result.cross.valid,false);assert.match(result.cross.error,/lexical scope/);assert.deepEqual(errors,[]);
 assert.ok(result.captures.includes('objects: Prepared(outer_cards)'));assert.ok(result.captures.includes('objects: Prepared(inner_cards)'));
 console.log(JSON.stringify({wasmNames:true,explicitUnusedResults:true,rename:true,irIdsUnchanged:true,strictDocumentImport:true,scopeIdentity:true,reflexiveTargets:true,activationModifierTargets:true,selectionCaptures:true,errors},null,2));await browser.close();
})().catch(async e=>{console.error(e);if(browser)await browser.close();process.exitCode=1});
