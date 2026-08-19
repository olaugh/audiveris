#!/usr/bin/env python3
"""Build the checked-in native HEADS template asset from the frozen Java oracle."""

from __future__ import annotations

import argparse
import hashlib
import math
import pathlib
import struct
import sys
from dataclasses import dataclass, field


MAGIC = b"AVHTPL01"
SHAPES = {
    "NOTEHEAD_BLACK": 0,
    "NOTEHEAD_VOID": 1,
    "WHOLE_NOTE": 2,
    "BREVE": 3,
}
ANCHORS = {
    "CENTER": 0,
    "MIDDLE_LEFT": 1,
    "MIDDLE_RIGHT": 2,
    "LEFT_STEM": 3,
    "TOP_LEFT_STEM": 4,
    "BOTTOM_LEFT_STEM": 5,
    "RIGHT_STEM": 6,
    "TOP_RIGHT_STEM": 7,
    "BOTTOM_RIGHT_STEM": 8,
}


@dataclass(frozen=True)
class Anchor:
    kind: int
    dx_bits: int
    dy_bits: int


@dataclass(frozen=True)
class Pixel:
    x: int
    y: int
    distance: int


@dataclass
class Template:
    shape: int
    point_size: int
    width: int
    height: int
    slim: tuple[int, int, int, int]
    expected_anchors: int
    expected_pixels: int
    anchors: list[Anchor] = field(default_factory=list)
    pixels: list[Pixel] = field(default_factory=list)

    def freeze(self) -> tuple[object, ...]:
        if self.width <= 0 or self.height <= 0:
            raise ValueError(f"shape {self.shape} has non-positive dimensions")
        sx, sy, sw, sh = self.slim
        if sx < 0 or sy < 0 or sw <= 0 or sh <= 0 or sx + sw > self.width or sy + sh > self.height:
            raise ValueError(f"shape {self.shape} has invalid slim bounds {self.slim}")
        if len(self.anchors) != self.expected_anchors:
            raise ValueError(
                f"shape {self.shape} has {len(self.anchors)} anchors, "
                f"expected {self.expected_anchors}"
            )
        if len(self.pixels) != self.expected_pixels:
            raise ValueError(
                f"shape {self.shape} has {len(self.pixels)} pixels, "
                f"expected {self.expected_pixels}"
            )
        if [anchor.kind for anchor in self.anchors] != list(range(len(self.anchors))):
            raise ValueError(f"shape {self.shape} anchors are not in enum order")
        expected_shape = self.shape
        expected_anchor_count = 9 if expected_shape < 2 else 3
        if len(self.anchors) != expected_anchor_count:
            raise ValueError(f"shape {self.shape} has the wrong active anchor set")
        if any(pixel.distance < -32768 or pixel.distance > 32767 for pixel in self.pixels):
            raise ValueError(f"shape {self.shape} distance does not fit signed 16 bits")
        previous: tuple[int, int] | None = None
        for pixel in self.pixels:
            if pixel.x < 0 or pixel.x >= self.width or pixel.y < 0 or pixel.y >= self.height:
                raise ValueError(f"shape {self.shape} pixel is outside the template: {pixel}")
            key = (pixel.y, pixel.x)
            if previous is not None and previous >= key:
                raise ValueError(f"shape {self.shape} pixels are not strictly row-major")
            previous = key
        return (
            self.shape,
            self.point_size,
            self.width,
            self.height,
            self.slim,
            tuple(self.anchors),
            tuple(self.pixels),
        )


def checked_i16(value: int, label: str) -> int:
    if value < -32768 or value > 32767:
        raise ValueError(f"{label} {value} does not fit signed 16 bits")
    return value


def java_round(value: float) -> int:
    return math.floor(value + 0.5)


def rounded_anchor(kind: int, dx: float, dy: float) -> tuple[int, int]:
    if kind in (1, 3, 4, 5):
        x = java_round(dx - 0.5 - 0.001)
    elif kind == 0:
        x = java_round(dx - 0.5)
    else:
        x = java_round(dx - 0.5 + 0.001)
    return x, java_round(dy - 0.5)


def parse_oracle(
    path: pathlib.Path, expected_shape: tuple[int, int] = (32, 8)
) -> tuple[bytes, dict[int, tuple[Template, ...]], int]:
    raw = path.read_bytes()
    page_catalogs: dict[tuple[str, int], list[Template]] = {}
    current_key: tuple[str, int] | None = None
    current: Template | None = None
    template_rows = 0

    def finish_template() -> None:
        nonlocal current
        if current is None or current_key is None:
            return
        current.freeze()
        page_catalogs.setdefault(current_key, []).append(current)
        current = None

    for line_number, raw_line in enumerate(raw.decode("utf-8").splitlines(), 1):
        fields = raw_line.split()
        if not fields or fields[0].startswith("#"):
            continue
        kind = fields[0]
        try:
            if kind == "headtemplate":
                finish_template()
                page = fields[1]
                catalog = int(fields[3])
                factory_ordinal = int(fields[5])
                active_ordinal = int(fields[7])
                shape = SHAPES[fields[9]]
                if fields[11] != "Bravura":
                    raise ValueError(f"unsupported family {fields[11]}")
                if factory_ordinal != shape or active_ordinal != shape:
                    raise ValueError("shape ordinals do not match the active factory order")
                point_size = int(fields[13])
                current_key = (page, catalog)
                current = Template(
                    shape=shape,
                    point_size=point_size,
                    width=checked_i16(int(fields[15]), "width"),
                    height=checked_i16(int(fields[17]), "height"),
                    slim=tuple(
                        checked_i16(int(value), "slim coordinate") for value in fields[19:23]
                    ),
                    expected_anchors=int(fields[24]),
                    expected_pixels=int(fields[26]),
                )
                template_rows += 1
            elif kind == "headtemplateanchor":
                if current is None:
                    raise ValueError("anchor outside a template")
                if (fields[1], int(fields[3])) != current_key:
                    raise ValueError("anchor page/catalog does not match current template")
                shape = SHAPES[fields[5]]
                ordinal = int(fields[7])
                anchor = ANCHORS[fields[9]]
                if shape != current.shape or ordinal != len(current.anchors) or anchor != ordinal:
                    raise ValueError("anchor does not match current template/order")
                dx_bits = int(fields[15], 16)
                dy_bits = int(fields[17], 16)
                if float.fromhex(fields[11]).hex() != struct.unpack(
                    ">d", struct.pack(">Q", dx_bits)
                )[0].hex():
                    raise ValueError("dx text and bits disagree")
                if float.fromhex(fields[13]).hex() != struct.unpack(
                    ">d", struct.pack(">Q", dy_bits)
                )[0].hex():
                    raise ValueError("dy text and bits disagree")
                dx = struct.unpack(">d", struct.pack(">Q", dx_bits))[0]
                dy = struct.unpack(">d", struct.pack(">Q", dy_bits))[0]
                if rounded_anchor(anchor, dx, dy) != (int(fields[19]), int(fields[20])):
                    raise ValueError("precise and rounded anchor offsets disagree")
                current.anchors.append(Anchor(anchor, dx_bits, dy_bits))
            elif kind == "headtemplatepixel":
                if current is None:
                    raise ValueError("pixel outside a template")
                if (fields[1], int(fields[3])) != current_key:
                    raise ValueError("pixel page/catalog does not match current template")
                shape = SHAPES[fields[5]]
                ordinal = int(fields[7])
                value = int(fields[13])
                bits = int(fields[17], 16)
                bit_value = struct.unpack(">d", struct.pack(">Q", bits))[0]
                if shape != current.shape or ordinal != len(current.pixels):
                    raise ValueError("pixel does not match current template/order")
                if bit_value != float(value) or bit_value.hex() != float.fromhex(fields[15]).hex():
                    raise ValueError("pixel integer, hex distance, and bits disagree")
                current.pixels.append(
                    Pixel(
                        checked_i16(int(fields[9]), "pixel x"),
                        checked_i16(int(fields[11]), "pixel y"),
                        checked_i16(value, "pixel distance"),
                    )
                )
        except (IndexError, KeyError, ValueError) as error:
            raise ValueError(f"{path}:{line_number}: {error}") from error

    finish_template()
    if (template_rows, len(page_catalogs)) != expected_shape:
        raise ValueError(
            f"expected {expected_shape[0]} templates in {expected_shape[1]} page catalogs, "
            f"got {template_rows} in {len(page_catalogs)}"
        )

    unique: dict[int, tuple[Template, ...]] = {}
    page_pixels = 0
    for page_key, templates in page_catalogs.items():
        if [template.shape for template in templates] != list(range(4)):
            raise ValueError(f"{page_key} templates are not in active factory order")
        if len({template.point_size for template in templates}) != 1:
            raise ValueError(f"{page_key} mixes point sizes")
        frozen = tuple(template.freeze() for template in templates)
        point_size = templates[0].point_size
        existing = unique.get(point_size)
        if existing is not None and tuple(item.freeze() for item in existing) != frozen:
            raise ValueError(f"point size {point_size} differs between page catalogs")
        unique.setdefault(point_size, tuple(templates))
        page_pixels += sum(len(template.pixels) for template in templates)

    return raw, unique, page_pixels


def encode(raw_oracle: bytes, catalogs: dict[int, tuple[Template, ...]]) -> bytes:
    output = bytearray(MAGIC)
    output.extend(hashlib.sha256(raw_oracle).digest())
    output.extend(struct.pack(">BB", 0, len(catalogs)))  # Bravura, catalog count

    for point_size in sorted(catalogs):
        templates = catalogs[point_size]
        output.extend(struct.pack(">hB", checked_i16(point_size, "point size"), len(templates)))
        for template in templates:
            sx, sy, sw, sh = template.slim
            output.extend(
                struct.pack(
                    ">BhhhhhhB",
                    template.shape,
                    template.width,
                    template.height,
                    sx,
                    sy,
                    sw,
                    sh,
                    len(template.anchors),
                )
            )
            for anchor in template.anchors:
                output.extend(struct.pack(">BQQ", anchor.kind, anchor.dx_bits, anchor.dy_bits))
            output.extend(struct.pack(">I", len(template.pixels)))
            for pixel in template.pixels:
                output.extend(struct.pack(">hhh", pixel.x, pixel.y, pixel.distance))
    return bytes(output)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("oracle", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument(
        "--small",
        type=pathlib.Path,
        default=None,
        help="explicit small point-size oracle (HeadTemplatePointSizeProbe output)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify that OUTPUT already contains the deterministic encoding",
    )
    args = parser.parse_args()

    raw, catalogs, page_pixels = parse_oracle(args.oracle)
    if sorted(catalogs) != [78, 83, 84, 85, 87] or page_pixels != 27207:
        raise ValueError(
            f"unexpected corpus coverage: point sizes {sorted(catalogs)}, pixels {page_pixels}"
        )
    if args.small is not None:
        # The page corpus cannot reach low-resolution point sizes (Java does
        # not always survive such scans end to end), so these catalogs come
        # from HeadTemplatePointSizeProbe: same TemplateFactory, explicit
        # sizes, one fresh JVM. Pinned exactly like the page corpus.
        small_raw, small_catalogs, small_pixels = parse_oracle(args.small, (28, 7))
        if sorted(small_catalogs) != [24, 25, 26, 27, 28, 29, 30] or small_pixels != 2495:
            raise ValueError(
                f"unexpected small coverage: point sizes {sorted(small_catalogs)}, "
                f"pixels {small_pixels}"
            )
        overlap = set(catalogs) & set(small_catalogs)
        if overlap:
            raise ValueError(f"small oracle repeats page-corpus sizes: {sorted(overlap)}")
        catalogs.update(small_catalogs)
        raw = raw + small_raw
    encoded = encode(raw, catalogs)
    if args.check:
        if not args.output.is_file() or args.output.read_bytes() != encoded:
            print(f"stale HEADS template asset: {args.output}", file=sys.stderr)
            return 1
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(encoded)

    unique_pixels = sum(
        len(template.pixels) for templates in catalogs.values() for template in templates
    )
    print(
        f"{len(catalogs)} catalogs, 20 templates, {unique_pixels} unique pixels, "
        f"{page_pixels} graded page pixels, {len(encoded)} bytes"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
