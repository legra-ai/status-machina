#!/usr/bin/env python3
"""Render checked executable graphs for GitHub and offline Rustdoc, using Graphviz."""
from __future__ import annotations

import html
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "docs/tutorials"


def graphviz(mermaid: str) -> str:
    """Translate the canonical state-diagram subset; refuse unsupported syntax."""
    result = ['digraph tutorial {', 'rankdir=LR;', 'bgcolor="#ffffff";',
              'graph [fontname="sans-serif", nodesep=0.5, ranksep=0.8];',
              'node [fontname="sans-serif", shape=box, style="rounded,filled", fillcolor="#eff6ff", color="#2563eb", fontcolor="#172554", margin="0.16,0.1"];',
              'edge [fontname="sans-serif", fontsize=10, color="#475569", fontcolor="#334155"];']
    for line in mermaid.splitlines():
        line = line.strip()
        if line == "stateDiagram-v2":
            continue
        state = re.fullmatch(r'state "(.*)" as (s\d+)', line)
        if state:
            result.append(f'{state[2]} [label={json.dumps(decode_label(state[1]), ensure_ascii=False)}];')
            continue
        terminal = re.fullmatch(r'(s\d+) --> \[\*\]', line)
        if terminal:
            result.extend([f'end_{terminal[1]} [shape=doublecircle, label="", width=0.12, height=0.12, fillcolor="#475569"];', f'{terminal[1]} -> end_{terminal[1]};'])
            continue
        initial = re.fullmatch(r'\[\*\] --> (s\d+)', line)
        if initial:
            result.extend(['start [shape=point, width=0.1, color="#475569"];', f'start -> {initial[1]};'])
            continue
        edge = re.fullmatch(r'(s\d+) --> (s\d+) : (.*)', line)
        if edge:
            result.append(f'{edge[1]} -> {edge[2]} [label={json.dumps(decode_label(edge[3]), ensure_ascii=False)}];')
            continue
        raise ValueError(f"Unsupported canonical graph syntax: {line}")
    return "\n".join([*result, "}", ""])


def decode_label(text: str) -> str:
    return re.sub(r'#(\d+);', lambda match: chr(int(match[1])), text)


def render(name: str, title: str, source: str) -> str:
    svg = subprocess.run(["dot", "-Tsvg"], input=graphviz(source), text=True,
                         check=True, capture_output=True).stdout
    svg = svg[svg.index("<svg "):]
    svg = re.sub(r"<svg\b[^>]*>", lambda match: " ".join(match[0].split()), svg, count=1)
    svg = re.sub(r"<(path|polygon|ellipse)(\s[^>]*?)/>", r"<\1\2></\1>", svg)
    svg = re.sub(r'<svg width="[^"]+" height="[^"]+"',
                 f'<svg width="100%" style="max-width: 72rem; height: auto" role="img" aria-labelledby="{name}-title {name}-description"', svg, count=1)
    # IDs must stay distinct when both images are inlined into one Rustdoc page.
    svg = re.sub(r'id="([^"]+)"', lambda match: f'id="{name}-{match[1]}"', svg)
    end = svg.index(">") + 1
    description = "Generated from the validated executable definition. Arrows name transitions, events and any guards; filled points mark entry and terminal states."
    metadata = f'\n<title id="{name}-title">{html.escape(title)}</title>\n<desc id="{name}-description">{description}</desc>\n<metadata id="{name}-source">{html.escape(source, quote=False)}</metadata>\n'
    # Inline HTML blocks are separated by blank lines in Markdown.
    return svg[:end] + metadata + svg[end:] + "\n"


def main() -> None:
    markdown = subprocess.run(["cargo", "run", "--quiet", "--example", "tutorial_diagrams"],
                              cwd=ROOT, check=True, text=True, capture_output=True).stdout
    graphs = re.findall(r'## ([^\n]+)\n\n```mermaid\n(.*?)```', markdown, re.DOTALL)
    if len(graphs) != 2:
        raise ValueError("Expected exactly the order and invoice graphs")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / "diagrams.md").write_text(markdown)
    for name, (title, source) in zip(("order", "invoice"), graphs, strict=True):
        (OUTPUT / f"{name}.svg").write_text(render(name, title, source))


if __name__ == "__main__":
    main()
