#!/usr/bin/env python3
"""Rebuild the original, geometric GUI concept kit using only the Python stdlib."""
import json
import re
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets' / 'gui'
DOC = ROOT / 'docs' / 'gui'
LABELS = dict(re.findall(r'"([a-z_]+)" => "([A-Za-z]+)"',
                         (ROOT / 'tui/src/lib.rs').read_text()))
THEME = {
    'background': '#151b20', 'panel': '#20292f', 'border': '#405059',
    'text': '#f1eee5', 'muted': '#bac4c8', 'squareLight': '#61716e',
    'squareDark': '#3e4d4c', 'accent': '#e2bd72', 'target': '#b4dfbf',
    'danger': '#f18e7f', 'p0': '#e2bd72', 'p1': '#82bcea',
    'p2': '#c1a0ee', 'p3': '#f18e7f',
}


def svg(w, h, title, body):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
            f'viewBox="0 0 {w} {h}" role="img" aria-labelledby="title">'
            f'<title id="title">{escape(title)}</title>{body}</svg>\n')


def token(label):
    return (f'<circle cx="32" cy="32" r="27" fill="{THEME["panel"]}" '
            f'stroke="currentColor" stroke-width="3"/>'
            f'<text x="32" y="40" text-anchor="middle" font-family="sans-serif" '
            f'font-size="23" font-weight="600" fill="currentColor">{escape(label)}</text>')


def text(x, y, value, size=16, color=None, weight=400):
    return (f'<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}" '
            f'font-weight="{weight}" fill="{color or THEME["text"]}">{escape(value)}</text>')


def rect(x, y, w, h, fill, radius=0, stroke='none'):
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{radius}" '
            f'fill="{fill}" stroke="{stroke}"/>')


def main():
    (OUT / 'pieces').mkdir(parents=True, exist_ok=True)
    (OUT / 'cards').mkdir(exist_ok=True)
    DOC.mkdir(parents=True, exist_ok=True)
    pieces = {}
    for path in sorted((ROOT / 'assets').glob('*.ron')):
        source = path.read_text()
        piece_id = re.search(r'\bid:\s*"([^"]+)"', source).group(1)
        name = re.search(r'display_name:\s*"([^"]+)"', source).group(1)
        label = LABELS[piece_id]
        dest = f'pieces/{piece_id}.svg'
        body = f'<g color="{THEME["text"]}">{token(label)}</g>'
        (OUT / dest).write_text(svg(64, 64, name, body))
        pieces[piece_id] = {'label': label, 'name': name, 'asset': dest}
    (OUT / 'pieces/unknown.svg').write_text(svg(64, 64, 'Unknown piece',
        f'<g color="{THEME["text"]}">{token("?")}</g>'))
    for kind, mark in [('summon', '+'), ('utility', '*'), ('onboard', 'O'), ('back', 'CC')]:
        body = rect(2, 2, 156, 216, THEME['panel'], 12, THEME['border'])
        body += rect(14, 14, 132, 100, THEME['background'], 6)
        body += f'<g color="{THEME["accent"]}" transform="translate(48 32)">{token(mark)}</g>'
        body += text(18, 148, 'CCHESS' if kind == 'back' else kind.upper(), 14, THEME['muted'])
        if kind != 'back':
            body += f'<path d="M18 168H142 M18 183H122 M18 198H134" stroke="{THEME["border"]}" stroke-width="3"/>'
        (OUT / f'cards/{kind}.svg').write_text(svg(160, 220, f'{kind} card template', body))
    icon = rect(0, 0, 128, 128, THEME['background'], 24)
    for y in range(2):
        for x in range(2):
            icon += rect(20+x*44, 20+y*44, 44, 44,
                         THEME['squareLight'] if (x+y)%2 else THEME['squareDark'], 3)
    icon += text(30, 79, 'CC', 46, THEME['accent'], 600)
    (OUT / 'app-icon.svg').write_text(svg(128, 128, 'CChess', icon))
    cards = {}
    for path in sorted((ROOT / 'assets/cards').glob('*.ron')):
        source = path.read_text()
        card_id = re.search(r'\bid:\s*"([^"]+)"', source).group(1)
        kind = re.search(r'\bkind:\s*(Summon|Utility|OnBoard)', source).group(1)
        cards[card_id] = {'template': f'cards/{kind.lower()}.svg'}
    (OUT / 'manifest.json').write_text(json.dumps({
        'version': 1, 'pieces': pieces, 'cards': cards,
        'fallback': 'pieces/unknown.svg', 'cardBack': 'cards/back.svg',
        'appIcon': 'app-icon.svg',
    }, indent=2) + '\n')
    (OUT / 'theme.css').write_text(':root {\n' + ''.join(
        f'  --cc-{re.sub(r"([A-Z])", lambda m: "-"+m[1].lower(), key)}: {value};\n'
        for key, value in THEME.items()) + '}\n')

    # Contact sheet: all catalogue tokens use precisely the distributed token art.
    body = rect(0, 0, 1050, 790, THEME['background'])
    body += text(30, 42, 'CCHESS / MINIMAL PIECE SET', 25, weight=600)
    body += text(30, 70, '36 archetypes · neutral vector tokens · owner color added at runtime', 16, THEME['muted'])
    for i, p in enumerate(pieces.values()):
        x, y = 28+(i%6)*171, 100+(i//6)*110
        body += f'<g transform="translate({x+44} {y})" color="{THEME["text"]}">{token(p["label"])}</g>'
        body += text(x+8, y+86, p['name'], 14)
    (DOC / 'asset-contact-sheet.svg').write_text(svg(1050, 790, 'All 36 minimal piece assets', body))

    # Deliberately illustrative, not claiming a legal engine snapshot.
    body = rect(0, 0, 1280, 920, THEME['background'])
    body += text(32, 43, 'CCHESS', 26, THEME['accent'], 600)
    body += text(182, 42, 'ARCANE TABLE', 14, THEME['muted'])
    body += text(850, 42, 'DESIGN CONCEPT · ILLUSTRATIVE POSITION', 13, THEME['muted'])
    body += rect(24, 66, 1232, 66, THEME['panel'], 10)
    body += text(44, 94, 'P0 · YOUR TURN', 15, THEME['accent'], 600)
    body += text(44, 118, 'Turn 12', 14, THEME['muted'])
    body += text(270, 105, 'Mana 3', 20)
    body += text(410, 105, 'Actions 1', 20)
    body += text(720, 105, 'P1 · Heuristic AI', 16, THEME['p1'])
    body += rect(1075, 81, 159, 36, THEME['accent'], 6)
    body += text(1111, 105, 'End turn', 16, THEME['background'], 600)
    bx, by, cell = 160, 166, 64
    for y in range(8):
        body += text(bx-23, by+y*cell+39, str(8-y), 14, THEME['muted'])
        for x in range(8):
            body += rect(bx+x*cell, by+y*cell, cell, cell,
                         THEME['squareLight'] if (x+y)%2 else THEME['squareDark'])
    for x in range(8):
        body += text(bx+x*cell+27, by+cell*8+24, chr(97+x), 14, THEME['muted'])
    # Candidate empty squares around a selected bishop, no simulated rule resolution.
    for x, y in [(2, 4), (4, 4), (3, 3), (3, 5)]:
        body += f'<circle cx="{bx+x*cell+32}" cy="{by+y*cell+32}" r="8" fill="{THEME["target"]}"/>'
    body += rect(bx+3*cell+2, by+4*cell+2, 60, 60, 'none', 0, THEME['accent'])
    placement = [(0,7,'Ro',0),(1,7,'Kn',0),(4,7,'Ki',0),(6,7,'Pl',0),
                 (1,6,'Pa',0),(2,6,'Pa',0),(5,6,'Pa',0),(6,6,'Pa',0),(3,4,'Bi',0),
                 (0,0,'Ro',1),(2,0,'Mg',1),(4,0,'Ki',1),(6,0,'Kn',1),
                 (0,1,'Pa',1),(2,1,'Pa',1),(5,1,'Pa',1),(6,1,'Pa',1),(5,3,'Dr',1)]
    for x,y,label,owner in placement:
        body += f'<g transform="translate({bx+x*cell} {by+y*cell})" color="{THEME[f"p{owner}"]}">{token(label)}</g>'
        if owner == 0:
            body += f'<circle cx="{bx+x*cell+51}" cy="{by+y*cell+52}" r="5" fill="{THEME["p0"]}"/>'
        else:
            px, py = bx+x*cell+51, by+y*cell+52
            body += f'<path d="M{px} {py-6}l6 6-6 6-6-6Z" fill="{THEME["p1"]}"/>'
    body += text(160, 736, '● Move target     □ Selected piece', 15, THEME['muted'])
    body += text(160, 762, 'Fit board    −    100%    +    Rotate', 15)
    body += rect(838, 154, 418, 622, THEME['panel'], 10)
    body += text(862, 186, 'SELECTED PIECE', 13, THEME['muted'])
    body += text(862, 220, 'Bishop', 30, weight=600)
    body += text(862, 247, 'P0 · d4 · Ready', 16, THEME['accent'])
    body += text(862, 286, 'Health 1 / 1', 18)
    body += text(862, 315, 'No active statuses', 15, THEME['muted'])
    body += text(862, 367, 'ABILITIES', 13, THEME['muted'])
    body += rect(862, 384, 370, 106, THEME['background'], 8, THEME['accent'])
    body += text(878, 414, 'Orthogonal step', 20)
    body += text(878, 442, '1 action · 0 mana', 16, THEME['accent'])
    body += text(878, 470, 'Choose an adjacent empty square', 15, THEME['muted'])
    body += text(862, 532, 'TARGETING', 13, THEME['muted'])
    body += text(862, 562, 'Select a highlighted square.', 17)
    body += text(862, 589, 'Review the result before confirming.', 15, THEME['muted'])
    body += text(862, 628, 'Cooldown after use: 2 owner turns', 15)
    body += rect(862, 652, 370, 40, THEME['border'], 6)
    body += text(1008, 678, 'Cancel', 16)
    body += text(862, 741, 'Events    History    On-board cards', 16)
    body += rect(24, 798, 1232, 98, THEME['panel'], 10)
    body += text(44, 826, 'YOUR HAND', 13, THEME['muted'])
    body += text(44, 856, '3 cards', 20)
    for i, (name, kind) in enumerate([('Summon Knight','SUMMON'),('Forced March','UTILITY'),('Fire','ON-BOARD')]):
        x = 205+i*265
        body += rect(x, 811, 246, 72, THEME['background'], 7, THEME['border'])
        body += text(x+14, 837, kind, 12, THEME['accent'])
        body += text(x+14, 864, name, 18)
    body += text(1030, 836, 'Undo · Save', 16)
    body += text(1030, 866, 'Match menu', 16, THEME['muted'])
    (DOC / 'match-concept.svg').write_text(svg(1280, 920, 'Arcane Table match interface concept', body))
    print(f'Generated {len(pieces)} piece tokens, {len(cards)} card mappings, 4 card templates and GUI concepts.')


if __name__ == '__main__':
    main()
