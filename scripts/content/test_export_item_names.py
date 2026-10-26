import pathlib, tempfile, unittest

from export_item_names import read_package


class ItemNamesExportTests(unittest.TestCase):
    def test_names_keep_translated_text_by_upper_case_key(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / 'heroes_text_english_eu.txt'
            lines = [
                '"lang"', '{', '"Tokens"', '{',
                '"HEROES_ITEM_NAME_LONGSWORD_REDMETAL" "Red Metal Longsword"',
                '"HEROES_ITEM_DESC_LONGSWORD_REDMETAL" "Line one\\nLine two"',
                '"HEROES_ITEM_NAME_GOLD" "Gold"',
                '"HEROES_ITEM_NAME_KOREAN_ONLY" "한글"',
                '"HEROES_SKILL_NAME_SKILL_FIONA_GUARD" "Guard"',
                '"HEROES_STATUS_NAME_HOTSPRING_POWDER_ROSE" "Rose Bath"',
                '"HEROES_QUEST_TITLE_X" "Not exported"',
                '}', '}',
            ]
            path.write_bytes(b'\xff\xfe' + '\r\n'.join(lines).encode('utf-16-le'))
            package = read_package(path)
        self.assertEqual(package['items']['LONGSWORD_REDMETAL'], ['Red Metal Longsword', 'Line one\nLine two'])
        self.assertEqual(package['items']['GOLD'], ['Gold', None])
        self.assertNotIn('KOREAN_ONLY', package['items'])
        self.assertEqual(package['skills'], {'SKILL_FIONA_GUARD': 'Guard'})
        self.assertEqual(package['status_effects'], {'HOTSPRING_POWDER_ROSE': 'Rose Bath'})


if __name__ == '__main__':
    unittest.main()
