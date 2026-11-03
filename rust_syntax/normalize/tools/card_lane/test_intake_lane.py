import json
from pathlib import Path
import tempfile
import unittest
import fcntl
from types import SimpleNamespace
from unittest.mock import patch
import intake_lane as intake

class IntakeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.root = Path(self.tmp.name)
        self.catalog = self.root/'catalog'; self.catalog.mkdir()
        (self.catalog/'scryfall-set.json').write_text(json.dumps({'code':'2ed','name':'Unlimited Edition'}))
        self.cards = {'Crusade':({'name':'Crusade','colors':['W'],'oracle_id':'1','oracle_text':'White creatures get +1/+1.'},1),
                      'Chaos Orb':({'name':'Chaos Orb','colors':[],'oracle_id':'2'},2),
                      'Contract from Below':({'name':'Contract from Below','colors':['B'],'oracle_id':'3','oracle_text':'Remove this card from your deck before playing if you are not playing for ante.'},3),
                      'Mixed':({'name':'Mixed','colors':['W','U'],'oracle_id':'4'},4)}
        (self.catalog/'scryfall-unlimited.json').write_text(json.dumps([{'name':n} for n in self.cards]))
        self.records = {n:[v] for n,v in self.cards.items()}
    def tearDown(self): self.tmp.cleanup()
    def test_selection_and_color(self):
        got = intake.select(self.records,['Contract from Below'],['Unlimited'],'white',self.catalog)
        self.assertEqual([n for n,_,_ in got],['Contract from Below','Crusade','Mixed'])
        self.assertEqual([n for n,_,_ in intake.select(self.records,[],['2ED'],'colorless',self.catalog)],['Chaos Orb'])
        self.assertEqual(intake.select(self.records,['crusade'],[],catalog=self.catalog)[0][0], 'Crusade')
        with self.assertRaises(ValueError): intake.select(self.records,['Crusad'],[],catalog=self.catalog)
        with self.assertRaises(ValueError): intake.local_set('unknown',self.catalog)
    def test_exclusions(self):
        self.assertIsNone(intake.exclusion(self.cards['Crusade'][0]))
        self.assertEqual(intake.exclusion(self.cards['Chaos Orb'][0]),'dexterity')
        self.assertEqual(intake.exclusion(self.cards['Contract from Below'][0]),'ante')
        self.assertEqual(intake.exclusion({'name':'Shahrazad', 'oracle_text':'Players play a MAGIC subgame, starting at 20 life.'}), 'subgame')
        self.assertEqual(intake.exclusion({'name':'Subgame on a face', 'card_faces':[{'oracle_text':'Start a subgame.'}]}), 'subgame')
        self.assertIsNone(intake.exclusion({'name':'Ordinary game', 'oracle_text':'You gain 2 life.'}))
    def test_files(self):
        p=self.root/'list'; p.write_text("Crusade\nChaos Orb\n")
        q=self.root/'json'; q.write_text('["Mixed"]')
        self.assertEqual(intake.files_names([p,q]),['Crusade','Chaos Orb','Mixed'])
    def test_cache_fetch_and_reuse(self):
        cache = self.root/'cache'
        calls = []
        def api(url):
            calls.append(url)
            if url.endswith('/sets'):
                return {'data':[{'code':'tst','name':'Test Set'}]}
            if len(calls) == 2:
                return {'data':[{'name':'Crusade'}], 'has_more':True,
                        'next_page':'https://api.scryfall.com/cards/search?page=2'}
            return {'data':[{'name':'Mixed'}], 'has_more':False}
        with patch.object(intake,'api_json',side_effect=api):
            result=intake.select(self.records,[],['Test Set'],catalog=self.catalog,cache=cache,fetch=True)
        self.assertEqual([n for n,_,_ in result],['Crusade','Mixed'])
        self.assertEqual(len(calls),3)
        with patch.object(intake,'api_json',side_effect=AssertionError('unexpected network')):
            self.assertEqual(len(intake.local_set('TST',self.catalog,cache)),2)
        self.assertIn('source',intake.read(cache/'tst.json'))
    def test_canonical_token_face_colors_and_non_ante_cards(self):
        self.records['Crusade'].append(({'name':'Crusade','layout':'token','oracle_id':'token'},9))
        self.records['Crusade'].append(({'name':'Crusade','layout':'front_card','oracle_id':'theme','type_line':'Card'},10))
        card = self.cards['Mixed'][0].copy()
        card.pop('colors'); card['card_faces']=[{'colors':['W']},{'colors':['U']}]
        self.records['Mixed']=[(card,4)]
        chosen=intake.select(self.records,['crusade'],['2ed'],'blue',self.catalog)
        self.assertEqual([n for n,_,_ in chosen],['Crusade','Mixed'])
        self.assertIsNone(intake.exclusion({'name':'Un-card','layout':'host','border_color':'silver','security_stamp':'acorn'}))
    def test_mtgjson_set(self):
        intake.save(self.catalog/'tst.json',{'data':{'code':'TST','name':'Test Set','cards':[{'name':'Crusade'}]}})
        self.assertEqual(intake.local_set('Test Set',self.catalog),[{'name':'Crusade'}])
    def test_plan_keyword_only_row_is_ready(self):
        lane = self.root/'lane'; (lane/'gen').mkdir(parents=True)
        (lane/'gen/rows.jsonl').write_text(json.dumps({'name':'Crusade','rust':None,'gap':None})+'\n')
        queue = self.root/'queue.json'; intake.save(queue,{'generator_commit':'abc','cards':[]})
        intake.save(self.root/'config.json',{'lane_root':str(lane),'implementation_queue':str(queue),'base':'def'})
        with patch.object(intake,'load_oracle',return_value=self.records), patch.object(intake,'implemented',return_value=set()):
            self.assertEqual(intake.plan(self.root,['Crusade'],[],None)[0]['readiness'],'pending')
    def test_plan_counts_support_from_the_validated_worker_snapshot(self):
        lane = self.root/'lane'; (lane/'gen').mkdir(parents=True)
        queue = self.root/'queue.json'; intake.save(queue, {'cards':[]})
        intake.save(self.root/'config.json', {'lane_root':str(lane), 'implementation_queue':str(queue), 'base':'old'})
        with patch.object(intake,'load_oracle',return_value=self.records), \
             patch('shared_base.read_work_base',return_value={'head':'validated'}), \
             patch.object(intake,'implemented',return_value={'Crusade'}) as support:
            self.assertEqual(intake.plan(self.root,['Crusade'],[],None)[0]['status'],'implemented')
        support.assert_called_once_with('validated',intake.CANTRIP)

    def test_add_preserves_queue_and_reconciles(self):
        queue=self.root/'queue.json'; intake.save(queue,{'generator_commit':'abc','cards':[{'name':'Old','status':'committed'}]})
        intake.save(self.root/'config.json',{'implementation_queue':str(queue),'base':'def'})
        item={'name':'Crusade','status':'candidate','readiness':'pending','layout':'normal','oracle_id':'1','oracle_source':'oracle','oracle_line':1}
        class FakeClaims:
            calls=0
            def __init__(self, records, holder): pass
            def claim(self,name):
                FakeClaims.calls += 1
                return {'issue':42,'holder':intake.HOLDER,'expires_at':'tomorrow'}
        with patch.object(intake,'load_oracle',return_value={}):
            intake.add(self.root,[item],FakeClaims)
            intake.add(self.root,[item],FakeClaims)
        data=intake.read(queue)
        self.assertEqual([c['name'] for c in data['cards']],['Old','Crusade'])
        self.assertEqual(data['cards'][0]['status'],'committed')
        self.assertEqual(data['cards'][1]['status'],'pending')
        self.assertEqual(intake.read(self.root/'intake-journal.json')['cards']['Crusade']['phase'],'enqueued')
    def test_other_claim_is_not_queued(self):
        q=self.root/'queue.json'; intake.save(q,{'generator_commit':'abc','cards':[]})
        intake.save(self.root/'config.json',{'implementation_queue':str(q),'base':'def'})
        class Held:
            def __init__(self,records,holder): pass
            def claim(self,name): raise RuntimeError('held by someone else')
        item={'name':'Crusade','status':'candidate','readiness':'pending','layout':'normal','oracle_id':'1','oracle_source':'oracle','oracle_line':1}
        with patch.object(intake,'load_oracle',return_value={}):
            with self.assertRaisesRegex(RuntimeError,'held by'): intake.add(self.root,[item],Held)
        self.assertEqual(intake.read(q)['cards'],[])

    def test_generator_gaps_are_catalogued_without_reservation(self):
        q=self.root/'queue.json';intake.save(q,{'generator_commit':'abc','cards':[]})
        intake.save(self.root/'config.json',{'implementation_queue':str(q),'base':'def'})
        claims=unittest.mock.Mock()
        item={'name':'Crusade','status':'candidate','readiness':'generator_gap','layout':'normal'}
        with patch.object(intake,'load_oracle',return_value={}):
            intake.add(self.root,[item],lambda *args:claims)
        claims.claim.assert_not_called()
        self.assertEqual(intake.read(q)['cards'],[])
        self.assertEqual(intake.read(self.root/'intake-journal.json')['cards']['Crusade']['phase'],'deferred_unclaimed')

    def test_incremental_claim_lookup_detects_a_late_unlabeled_issue(self):
        raw={'number':7,'title':'Card: Crusade','state':'closed','labels':[]}
        ci=SimpleNamespace(gh=unittest.mock.Mock(return_value=[[raw]]),
                           with_events=lambda x:x,read_state=unittest.mock.Mock(return_value={7:{**raw,'state':'CLOSED'}}),
                           title=lambda name:'Card: '+name)
        claims=intake.Claims.__new__(intake.Claims)
        claims.ci,claims.state,claims.since,claims.holder=ci,{},'2026-11-03T00:00:00Z',intake.HOLDER
        with self.assertRaisesRegex(RuntimeError,'closed card issue #7'):
            claims.claim('Crusade')
        self.assertEqual(ci.gh.call_count,1)
        ci.read_state.assert_called_once_with(intake.REPO,fresh=True)

    def test_incremental_lookup_keeps_full_lease_refresh_for_existing_claims(self):
        issue={'number':7,'title':'Card: Crusade','state':'OPEN'}
        ci=SimpleNamespace(gh=unittest.mock.Mock(return_value=[[]]),title=lambda name:'Card: '+name,
                           read_state=unittest.mock.Mock(return_value={7:issue}),read_issue=lambda *args:issue,
                           holder=lambda issue:('someone/worker',None,None))
        claims=intake.Claims.__new__(intake.Claims)
        claims.ci,claims.state,claims.since,claims.holder=ci,{7:issue},'2026-11-03T00:00:00Z',intake.HOLDER
        with self.assertRaisesRegex(RuntimeError,'held by someone/worker'):
            claims.claim('Crusade')
        self.assertEqual(ci.gh.call_count,1)
        ci.read_state.assert_called_once_with(intake.REPO,fresh=True)

    def test_reservation_does_not_lock_out_implementation_outcomes(self):
        q=self.root/'queue.json'; intake.save(q,{'generator_commit':'abc','cards':[]})
        intake.save(self.root/'config.json',{'implementation_queue':str(q),'base':'def'})
        class Claims:
            def __init__(self,*args):pass
            def claim(self,name):
                with Path(str(q)+'.lock').open('a') as lock:
                    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
                return {'issue':42}
        item={'name':'Crusade','status':'candidate','readiness':'pending','layout':'normal','oracle_id':'1','oracle_source':'oracle','oracle_line':1}
        with patch.object(intake,'load_oracle',return_value={}):
            intake.add(self.root,[item],Claims)
        self.assertEqual(intake.read(q)['cards'][0]['name'],'Crusade')

if __name__ == '__main__': unittest.main()
