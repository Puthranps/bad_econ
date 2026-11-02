import gzip
import hashlib
import json
from pathlib import Path
import subprocess

folder = Path('target/society-evidence')
baseline = json.loads((folder / 'baseline.json').read_text(encoding='utf-8'))
cases = {
    'baseline': {},
    'reciprocity-25': {'reciprocity_percent': 25},
    'reciprocity-75': {'reciprocity_percent': 75},
    'escrow': {'trade_rule': 'Escrow'},
    'posted-12': {'wage_offer': 2, 'reservation_wage': 4, 'maximum_wage': 8, 'work_food': 12, 'wage_rule': 'Posted'},
    'posted-18': {'wage_offer': 2, 'reservation_wage': 4, 'maximum_wage': 8, 'work_food': 18, 'wage_rule': 'Posted'},
    'negotiated-12': {'wage_offer': 2, 'reservation_wage': 4, 'maximum_wage': 8, 'work_food': 12, 'wage_rule': 'Negotiate'},
    'negotiated-18': {'wage_offer': 2, 'reservation_wage': 4, 'maximum_wage': 8, 'work_food': 18, 'wage_rule': 'Negotiate'},
}
summaries = []
for name, patch in cases.items():
    scenario = baseline['scenario'] | patch
    scenario_path = folder / f'{name}-scenario.json'
    scenario_path.write_text(json.dumps(scenario, indent=2) + '\n', encoding='utf-8')
    report_path = folder / f'{name}.json'
    with (folder / f'{name}-run.log').open('w', encoding='utf-8') as log:
        subprocess.run(['target/debug/bad_econ.exe', 'society', '--scenario', str(scenario_path), '--backend', 'gpu', '--days', '120', '--output', str(report_path)], check=True, stdout=log, stderr=subprocess.STDOUT)
    report = json.loads(report_path.read_text(encoding='utf-8'))
    config = report['scenario']
    groups = (config['capacity'] + config['neighborhood_size'] - 1) // config['neighborhood_size']
    food = len(config['agents']) * config['initial_food'] + groups * config['firm_food']
    cash = len(config['agents']) * config['initial_cash'] + groups * config['firm_cash']
    population = len(config['agents'])
    deceased = set()
    for day in report['rounds']:
        summary = day['summary']
        living = [a for a in day['agents'] if a['life'] in ('Adult', 'Child')]
        assert len(living) == summary['population']
        assert len({a['identity'] for a in living}) == len(living)
        assert not deceased.intersection(a['identity'] for a in living)
        deceased.update(a['identity'] for a in day['agents'] if a['life'] == 'Dead')
        assert all(not a['employed'] for a in day['agents'] if a['life'] == 'Child')
        assert sum(a['cash'] for a in day['agents']) + sum(f['cash'] for f in day['firms']) == cash == summary['cash']
        assert sum(a['food'] for a in day['agents']) + sum(f['food'] for f in day['firms']) == summary['food_reserves']
        assert food + summary['produced_food'] == summary['food_reserves'] + summary['consumed_food'] + summary['abandoned_food']
        assert population + summary['births'] - summary['deaths'] == summary['population']
        food, population = summary['food_reserves'], summary['population']
    days = report['rounds']
    worker_days = sum(d['summary']['employed'] for d in days)
    wages = sum(a['wage'] for d in days for a in d['agents'])
    compact = json.dumps(report, separators=(',', ':')).encode()
    (folder / f'{name}.json.gz').write_bytes(gzip.compress(compact, mtime=0))
    summaries.append({
        'case': name,
        'settings_changed': patch,
        'device': report['device'],
        'days': len(days),
        'starting_population': len(config['agents']),
        'peak_population': max(d['summary']['population'] for d in days),
        'final_population': population,
        'births': sum(d['summary']['births'] for d in days),
        'deaths': sum(d['summary']['deaths'] for d in days),
        'frauds': sum(d['summary']['frauds'] for d in days),
        'trades': sum(d['summary']['trades'] for d in days),
        'unmet_food_agent_days': sum(d['summary']['hungry_agents'] for d in days),
        'worker_days': worker_days,
        'average_wage_when_employed': wages / max(1, worker_days),
        'newborn_identities_later_employed': len({a['identity'] for d in days for a in d['agents'] if a['parent_identity'] > 0 and a['employed']}),
        'final_firm_cash': sum(f['cash'] for f in days[-1]['firms']),
        'final_food': food,
        'cash_every_day': cash,
        'report_sha256': hashlib.sha256(compact).hexdigest(),
    })
    print(json.dumps(summaries[-1]), flush=True)
(folder / 'summary.json').write_text(json.dumps({'model': baseline['model'], 'seed_for_starting_population': 42, 'cases': summaries}, indent=2) + '\n', encoding='utf-8')
