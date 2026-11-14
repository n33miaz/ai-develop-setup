"""Generate the provisional shell icon (icon.png and icon.ico) with the standard library only.

Usage: python tools/gen_shell_icon.py [output_dir]
The drawing is original: a rounded square with a lighter disc. Replace it with the final art later.
"""

import struct
import sys
import zlib
from pathlib import Path

BACKGROUND = (30, 41, 59, 255)
DISC = (56, 189, 248, 255)
CLEAR = (0, 0, 0, 0)


def pixel(x, y, size):
    # Coordinates in [0, 1); rounded square with radius 0.22 and a centered disc of radius 0.26.
    u, v = (x + 0.5) / size, (y + 0.5) / size
    cx, cy, r = min(max(u, 0.22), 0.78), min(max(v, 0.22), 0.78), 0.22
    if (u - cx) ** 2 + (v - cy) ** 2 > r * r:
        return CLEAR
    if (u - 0.5) ** 2 + (v - 0.5) ** 2 <= 0.26**2:
        return DISC
    return BACKGROUND


def raster(size):
    return [[pixel(x, y, size) for x in range(size)] for y in range(size)]


def chunk(tag, data):
    body = tag + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


def png(size):
    rows = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in raster(size))
    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(rows, 9))
        + chunk(b"IEND", b"")
    )


def ico_entry(size):
    # Classic BMP entry (32-bit BGRA, bottom-up, plus an all-zero AND mask).
    pixels = b"".join(
        bytes((b, g, r, a)) for row in reversed(raster(size)) for (r, g, b, a) in row
    )
    mask = b"\x00" * (((size + 31) // 32) * 4 * size)
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, len(pixels) + len(mask), 0, 0, 0, 0)
    return header + pixels + mask


def ico(sizes):
    images = [ico_entry(s) for s in sizes]
    offset = 6 + 16 * len(sizes)
    out = struct.pack("<HHH", 0, 1, len(sizes))
    for s, img in zip(sizes, images):
        out += struct.pack("<BBBBHHII", s, s, 0, 0, 1, 32, len(img), offset)
        offset += len(img)
    return out + b"".join(images)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "crates/shell/icons")
    out.mkdir(parents=True, exist_ok=True)
    (out / "icon.png").write_bytes(png(128))
    (out / "icon.ico").write_bytes(ico([16, 32, 48]))


if __name__ == "__main__":
    main()
