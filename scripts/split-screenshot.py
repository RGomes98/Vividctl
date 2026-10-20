#!/usr/bin/env python3
"""Joins two screenshots of the same screen into one image: the left half from the first
(vividctl off), the right half from the second (vividctl on), for documentation.

    scripts/split-screenshot.py off.png on.png split.png [--crop-top PX] [--crop-bottom PX]
                                                         [--divider PX] [--labels]
                                                         [--label-off TEXT] [--label-on TEXT] [--quality N]

A `.jpg` output is written at quality 100 without chroma subsampling, the best JPEG can do; `--quality` lowers it.
Both screenshots must have the same size. Take them one after the other with nothing changing on
screen, and crop the top bar and the dock away if their clock or icons differ.

Needs Python 3 with Pillow (`sudo apt install python3-pil`).
"""

import argparse
import sys

from PIL import Image, ImageDraw, ImageFont


def split(off: Image.Image, on: Image.Image, divider: int) -> Image.Image:
    """The left half of `off` next to the right half of `on`, with a vertical line between."""
    if off.size != on.size:
        raise ValueError(f"the screenshots differ in size: {off.size} and {on.size}")
    width, height = off.size
    middle = width // 2
    joined = Image.new("RGB", off.size)
    joined.paste(off.crop((0, 0, middle, height)), (0, 0))
    joined.paste(on.crop((middle, 0, width, height)), (middle, 0))
    if divider > 0:
        ImageDraw.Draw(joined).rectangle(
            (middle - divider // 2, 0, middle - divider // 2 + divider - 1, height), fill=(255, 255, 255)
        )
    return joined


def label(image: Image.Image, text: str, corner: str) -> None:
    """A small caption on a dark plate, in the bottom left or bottom right corner."""
    width, height = image.size
    size = max(14, height // 40)
    try:
        font = ImageFont.truetype("DejaVuSans-Bold.ttf", size)
    except OSError:
        font = ImageFont.load_default()
    draw = ImageDraw.Draw(image)
    left, top, right, bottom = draw.textbbox((0, 0), text, font=font)
    pad = size // 2
    plate_width, plate_height = right - left + 2 * pad, bottom - top + 2 * pad
    x = pad if corner == "left" else width - plate_width - pad
    y = height - plate_height - pad
    draw.rounded_rectangle((x, y, x + plate_width, y + plate_height), radius=pad, fill=(0, 0, 0))
    draw.text((x + pad - left, y + pad - top), text, font=font, fill=(255, 255, 255))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("off", help="screenshot with vividctl off (neutral)")
    parser.add_argument("on", help="screenshot with vividctl on")
    parser.add_argument("output", help="where to write the joined image")
    parser.add_argument("--crop-top", type=int, default=0, metavar="PX", help="pixels to cut from the top (the top bar)")
    parser.add_argument("--crop-bottom", type=int, default=0, metavar="PX", help="pixels to cut from the bottom (a dock)")
    parser.add_argument("--divider", type=int, default=2, metavar="PX", help="width of the line between the halves, 0 for none")
    parser.add_argument("--labels", action="store_true", help="write a caption in each bottom corner")
    parser.add_argument("--label-off", default="vividctl off", metavar="TEXT", help="caption of the left half (default: 'vividctl off')")
    parser.add_argument("--label-on", default="vividctl on", metavar="TEXT", help="caption of the right half, such as 'vividctl 425' (default: 'vividctl on')")
    parser.add_argument("--quality", type=int, default=100, metavar="N", help="JPEG quality, 1 to 100 (default: 100); ignored for other formats")
    args = parser.parse_args()

    try:
        off, on = Image.open(args.off).convert("RGB"), Image.open(args.on).convert("RGB")
    except OSError as error:
        print(f"error: cannot read a screenshot: {error}", file=sys.stderr)
        return 1
    if off.size != on.size:
        print(f"error: the screenshots differ in size: {off.size} and {on.size}", file=sys.stderr)
        return 1
    width, height = off.size
    if args.crop_top < 0 or args.crop_bottom < 0 or args.crop_top + args.crop_bottom >= height:
        print(f"error: cannot crop {args.crop_top} + {args.crop_bottom} pixels from a {height} pixel tall image", file=sys.stderr)
        return 1
    box = (0, args.crop_top, width, height - args.crop_bottom)
    joined = split(off.crop(box), on.crop(box), args.divider)
    if args.labels:
        label(joined, args.label_off, "left")
        label(joined, args.label_on, "right")
    if args.output.lower().endswith((".jpg", ".jpeg")):
        joined.save(args.output, quality=args.quality, subsampling=0, optimize=True, progressive=True)
    else:
        joined.save(args.output)
    print(f"wrote {args.output} ({joined.size[0]}x{joined.size[1]})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
