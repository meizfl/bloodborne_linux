#!/usr/bin/env python3
"""Compare real client-area framebuffer captures of reference and Rust pages."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops, ImageStat


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('reference', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    pages = []
    for path in sorted(args.reference.glob('*.png')):
        candidate = args.candidate / (path.stem + '.ppm')
        reference = Image.open(path).convert('RGB')
        actual = Image.open(candidate).convert('RGB')
        if reference.size != actual.size:
            pages.append({'page': path.stem, 'equal': False, 'sizes': [reference.size, actual.size]})
            continue
        delta = ImageChops.difference(reference, actual)
        pages.append({'page': path.stem, 'equal': delta.getbbox() is None,
                      'mean_channel_error': sum(ImageStat.Stat(delta).mean) / 3})
    if not pages:
        raise SystemExit('No reference pages')
    result = {'pages': pages, 'all_equal': all(page['equal'] for page in pages)}
    if args.report:
        args.report.write_text(json.dumps(result, indent=2))
    print(json.dumps(result))
    raise SystemExit(0 if result['all_equal'] else 1)


if __name__ == '__main__': main()
