import os
import json

locales_dir = 'src/locales'
languages = os.listdir(locales_dir)

for lang in languages:
    common_path = os.path.join(locales_dir, lang, 'common.json')
    if os.path.isfile(common_path):
        with open(common_path, 'r', encoding='utf-8') as f:
            data = json.load(f)

        if 'settings' in data and 'advanced' in data['settings']:
            adv = data['settings']['advanced']

            if 'loopPreventionThresholdPlaceholder' not in adv:
                if lang == 'en':
                    adv['loopPreventionThresholdPlaceholder'] = 'e.g., 3'
                elif lang == 'ko':
                    adv['loopPreventionThresholdPlaceholder'] = '예: 3'
                elif lang == 'de':
                    adv['loopPreventionThresholdPlaceholder'] = 'z. B. 3'
                elif lang == 'es':
                    adv['loopPreventionThresholdPlaceholder'] = 'p. ej., 3'
                elif lang == 'fr':
                    adv['loopPreventionThresholdPlaceholder'] = 'ex. 3'
                elif lang == 'ja':
                    adv['loopPreventionThresholdPlaceholder'] = '例: 3'
                elif lang == 'pt':
                    adv['loopPreventionThresholdPlaceholder'] = 'ex., 3'
                elif lang == 'zh':
                    adv['loopPreventionThresholdPlaceholder'] = '例如，3'
                else:
                    adv['loopPreventionThresholdPlaceholder'] = 'e.g., 3'

        with open(common_path, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
            f.write('\n')
