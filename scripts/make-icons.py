"""Generate the desktop app icons from the Harmony mark (dark rounded square,
orange clock ring). Python stdlib only. Outputs:

  desktop/icons/harmony-64.rgba   raw RGBA, 64x64, for the window icon
  desktop/icons/harmony.ico       PNG-payload ICO (16/32/64/256) for the exe

Run from the repo root: python scripts/make-icons.py
"""
import struct
import zlib
from pathlib import Path

BG = (0x0F, 0x11, 0x15)
FG = (0xF9, 0x73, 0x16)
OUT = Path(__file__).resolve().parent.parent / "desktop" / "icons"


def coverage(px, py, size, ss=4):
    """Return (bg_cov, fg_cov) in 0..1 for pixel (px, py) by supersampling."""
    bg = fg = 0
    n = ss * ss
    for sy in range(ss):
        for sx in range(ss):
            x = (px + (sx + 0.5) / ss) / size * 32.0  # work in the 32-unit favicon space
            y = (py + (sy + 0.5) / ss) / size * 32.0
            # rounded square, radius 7 (matches favicon.svg)
            r = 7.0
            cx = min(max(x, r), 32 - r)
            cy = min(max(y, r), 32 - r)
            inside = (x - cx) ** 2 + (y - cy) ** 2 <= r * r
            if not inside:
                continue
            bg += 1
            dx, dy = x - 16, y - 16
            d = (dx * dx + dy * dy) ** 0.5
            ring = 8.5 <= d <= 11.5  # circle r=10, stroke 3
            hand1 = abs(dx) <= 1.5 and 9 <= y <= 16  # 12 o'clock hand
            # hand to 4 o'clock: segment (16,16)-(21,19), width 3
            ex, ey = 5.0, 3.0
            t = max(0.0, min(1.0, (dx * ex + dy * ey) / (ex * ex + ey * ey)))
            hx, hy = dx - t * ex, dy - t * ey
            hand2 = (hx * hx + hy * hy) ** 0.5 <= 1.5
            if ring or hand1 or hand2:
                fg += 1
    return bg / n, fg / n


def render(size):
    buf = bytearray()
    for py in range(size):
        for px in range(size):
            bgc, fgc = coverage(px, py, size)
            if bgc == 0:
                buf += bytes((0, 0, 0, 0))
                continue
            # composite fg over bg, alpha = bg coverage
            fgf = fgc / bgc
            r = round(BG[0] * (1 - fgf) + FG[0] * fgf)
            g = round(BG[1] * (1 - fgf) + FG[1] * fgf)
            b = round(BG[2] * (1 - fgf) + FG[2] * fgf)
            buf += bytes((r, g, b, round(255 * bgc)))
    return bytes(buf)


def png(rgba, size):
    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + rgba[y * size * 4 : (y + 1) * size * 4] for y in range(size))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def ico(pngs):
    header = struct.pack("<HHH", 0, 1, len(pngs))
    offset = 6 + 16 * len(pngs)
    entries = b""
    for size, data in pngs:
        entries += struct.pack(
            "<BBBBHHII", size if size < 256 else 0, size if size < 256 else 0, 0, 0, 1, 32, len(data), offset
        )
        offset += len(data)
    return header + entries + b"".join(d for _, d in pngs)


OUT.mkdir(parents=True, exist_ok=True)
(OUT / "harmony-64.rgba").write_bytes(render(64))
(OUT / "harmony.ico").write_bytes(ico([(s, png(render(s), s)) for s in (16, 32, 64, 256)]))
print("wrote", OUT / "harmony-64.rgba", "and", OUT / "harmony.ico")
