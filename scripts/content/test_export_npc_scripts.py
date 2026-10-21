import contextlib, pathlib, sqlite3, unittest
import test_export_fiona_inventory as inventory_tests
from export_npc_scripts import read_package, digest

COLUMNS = 'StoryLineID TEXT,StoryLineName TEXT,XmlPath TEXT,Npc TEXT,Category TEXT,Stage TEXT,RewardGold INT,RewardExp INT,RewardAp INT,RewardTitle INT,RewardItem TEXT,RewardItemCount INT,RewardSkill TEXT,RewardSkillRank INT,AutoGiveLevel INT,ClassRestriction INT,AutoProcess bit,AutoReveal bit,Season INT,Episode TEXT,Feature TEXT'

class NpcScriptExportTests(unittest.TestCase):
    def setUp(self):
        self.helper = inventory_tests.FionaInventoryExportTests(); self.helper.setUp(); self.addCleanup(self.helper.doCleanups)
        self.db = self.helper.db; self.config = self.helper.config
        self.xml = pathlib.Path(self.helper.tmp.name) / 'XML'; (self.xml / 'Default').mkdir(parents=True); (self.xml / 'QuestSet01').mkdir()
        (self.xml / 'NpcTalkInfo.xml').write_text('<?xml version="1.0"?>\n<FileList>\n  <Default>Default\\Default_A</Default>\n  <Default Feature="flag">Default\\Default_B</Default>\n</FileList>', encoding='utf-8-sig')
        (self.xml / 'Default' / 'Default_A.xml').write_text('<DefaultPhase>\n  <ButtonSet id="base_buttonset"><Button caption="bye" command="endtalk"/></ButtonSet>\n  <Talk id="FERGHUS" location="t01_bs" command="greeting">\n    <Do>\n      <talk>HELLO\\nTHERE</talk>\n      <change_npc>FERGHUS, gerens</change_npc>\n    </Do>\n  </Talk>\n</DefaultPhase>', encoding='utf-8')
        (self.xml / 'Default' / 'Default_B.xml').write_text('<DefaultPhase><Talk id="ARIEL" command="greeting"><Condition><a/>text</Condition></Talk></DefaultPhase>', encoding='utf-8')
        (self.xml / 'QuestSet01' / 'GuidePlay.xml').write_text('<StoryLine><Phase id="p0"><Goal>  </Goal></Phase></StoryLine>', encoding='utf-8')
        with contextlib.closing(sqlite3.connect(self.db)) as db:
            db.execute(f'CREATE TABLE StoryLineInfo({COLUMNS})')
            db.execute('INSERT INTO StoryLineInfo VALUES(' + ','.join('?' * 21) + ')', ('GuidePlay', 'n', 'QuestSet01', None, 'MainStream', None, 0, 0, 0, 0, None, '', None, 0, 1, 1023, 'False', 'True', 1, None, None))
            db.commit()
    def test_trees_keep_order_tails_and_source_integer_reads(self):
        before = [digest(p) for p in (self.db, self.config)]
        p = read_package(self.xml, self.db, self.config)
        self.assertEqual([(d['feature'], d['path']) for d in p['defaults']], [('', 'Default\\Default_A'), ('flag', 'Default\\Default_B')])
        talk = p['defaults'][0]['tree'][3][1]
        self.assertEqual((talk[0], talk[1]), ('Talk', [['id', 'FERGHUS'], ['location', 't01_bs'], ['command', 'greeting']]))
        self.assertEqual(talk[3][0][3][0][2], 'HELLO\\nTHERE')
        condition = p['defaults'][1]['tree'][3][0][3][0]
        self.assertEqual((condition[3][0][4], p['storylines'][0]['tree'][3][0][3][0][2]), ('text', ''))
        info = p['storylines'][0]['info']
        self.assertEqual((info['RewardItemCount'], info['AutoReveal'], info['AutoProcess'], p['manifest']['coerced_integers']), (0, True, False, 1))
        self.assertEqual(before, [digest(p) for p in (self.db, self.config)])
    def test_missing_storyline_xml_is_refused(self):
        (self.xml / 'QuestSet01' / 'GuidePlay.xml').unlink()
        with self.assertRaises(OSError): read_package(self.xml, self.db, self.config)

if __name__ == '__main__':
    unittest.main()
