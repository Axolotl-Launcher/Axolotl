#!/usr/bin/env python3
"""Optically center the RGBA images stored in a Windows ICO file."""

from __future__ import annotations

import argparse
import math
import struct
import zlib
from pathlib import Path


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_chunks(data: bytes):
    position = 8
    while position < len(data):
        length = struct.unpack_from(">I", data, position)[0]
        chunk_type = data[position + 4 : position + 8]
        chunk_data = data[position + 8 : position + 8 + length]
        position += length + 12
        yield chunk_type, chunk_data
        if chunk_type == b"IEND":
            break


def decode_png(data: bytes) -> tuple[int, int, bytearray]:
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError("ICO entry is not a PNG image")

    width, height, bit_depth, color_type, compression, filter_method, interlace = struct.unpack(
        ">IIBBBBB", data[16:29]
    )
    if (bit_depth, color_type, compression, filter_method, interlace) != (8, 6, 0, 0, 0):
        raise ValueError(
            "only non-interlaced 8-bit RGBA PNG entries are supported "
            f"(got bit_depth={bit_depth}, color_type={color_type})"
        )

    idat = b"".join(chunk_data for chunk_type, chunk_data in png_chunks(data) if chunk_type == b"IDAT")
    decoded = zlib.decompress(idat)
    stride = width * 4
    rows = bytearray(width * height * 4)
    previous = bytearray(stride)
    position = 0

    for y in range(height):
        filter_type = decoded[position]
        position += 1
        row = bytearray(decoded[position : position + stride])
        position += stride
        for index in range(stride):
            left = row[index - 4] if index >= 4 else 0
            up = previous[index]
            up_left = previous[index - 4] if index >= 4 else 0
            if filter_type == 1:
                row[index] = (row[index] + left) & 0xFF
            elif filter_type == 2:
                row[index] = (row[index] + up) & 0xFF
            elif filter_type == 3:
                row[index] = (row[index] + ((left + up) // 2)) & 0xFF
            elif filter_type == 4:
                prediction = left + up - up_left
                left_error = abs(prediction - left)
                up_error = abs(prediction - up)
                up_left_error = abs(prediction - up_left)
                if left_error <= up_error and left_error <= up_left_error:
                    predictor = left
                elif up_error <= up_left_error:
                    predictor = up
                else:
                    predictor = up_left
                row[index] = (row[index] + predictor) & 0xFF
            elif filter_type != 0:
                raise ValueError(f"unsupported PNG filter type {filter_type}")
            rows[y * stride : (y + 1) * stride] = row
        previous = row

    return width, height, rows


def encode_png(width: int, height: int, rgba: bytes) -> bytes:
    def chunk(chunk_type: bytes, chunk_data: bytes) -> bytes:
        return struct.pack(">I", len(chunk_data)) + chunk_type + chunk_data + struct.pack(
            ">I", zlib.crc32(chunk_type + chunk_data) & 0xFFFFFFFF
        )

    raw = b"".join(b"\x00" + rgba[y * width * 4 : (y + 1) * width * 4] for y in range(height))
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return PNG_SIGNATURE + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def visual_center(width: int, height: int, rgba: bytes) -> tuple[float, float]:
    alpha_sum = 0
    weighted_x = 0
    weighted_y = 0
    for y in range(height):
        for x in range(width):
            alpha = rgba[(y * width + x) * 4 + 3]
            alpha_sum += alpha
            weighted_x += x * alpha
            weighted_y += y * alpha
    if not alpha_sum:
        raise ValueError("PNG entry has no visible pixels")
    return weighted_x / alpha_sum, weighted_y / alpha_sum


def shift_rgba(width: int, height: int, rgba: bytes, dx: int, dy: int) -> bytes:
    shifted = bytearray(len(rgba))
    for y in range(height):
        for x in range(width):
            target_x = x + dx
            target_y = y + dy
            if 0 <= target_x < width and 0 <= target_y < height:
                source = (y * width + x) * 4
                target = (target_y * width + target_x) * 4
                shifted[target : target + 4] = rgba[source : source + 4]
    return bytes(shifted)


def center_entry(data: bytes) -> tuple[bytes, int, int, tuple[float, float], tuple[float, float]]:
    width, height, rgba = decode_png(data)
    old_center = visual_center(width, height, rgba)
    dx = math.floor((width - 1) / 2 - old_center[0] + 0.5)
    dy = math.floor((height - 1) / 2 - old_center[1] + 0.5)
    shifted = shift_rgba(width, height, rgba, dx, dy)
    new_center = visual_center(width, height, shifted)
    return encode_png(width, height, shifted), dx, dy, old_center, new_center


def rewrite_ico(input_path: Path, output_path: Path) -> None:
    data = input_path.read_bytes()
    reserved, image_type, count = struct.unpack_from("<HHH", data, 0)
    if (reserved, image_type) != (0, 1):
        raise ValueError("input is not an icon ICO file")

    entries = []
    for index in range(count):
        offset = 6 + index * 16
        entry = struct.unpack_from("<BBBBHHII", data, offset)
        width, height, colors, reserved_byte, planes, bit_count, size, image_offset = entry
        image = data[image_offset : image_offset + size]
        if image.startswith(PNG_SIGNATURE):
            new_image, dx, dy, old_center, new_center = center_entry(image)
            label = width or 256
            print(
                f"{label}x{height or 256}: shift ({dx:+d}, {dy:+d}), "
                f"center ({old_center[0]:.2f}, {old_center[1]:.2f}) -> "
                f"({new_center[0]:.2f}, {new_center[1]:.2f})"
            )
        else:
            new_image = image
            print(f"{width or 256}x{height or 256}: unchanged non-PNG entry")
        entries.append((width, height, colors, reserved_byte, planes, bit_count, new_image))

    first_image_offset = 6 + 16 * count
    output = bytearray(struct.pack("<HHH", reserved, image_type, count))
    image_offset = first_image_offset
    images = []
    for width, height, colors, reserved_byte, planes, bit_count, image in entries:
        output.extend(
            struct.pack(
                "<BBBBHHII",
                width,
                height,
                colors,
                reserved_byte,
                planes,
                bit_count,
                len(image),
                image_offset,
            )
        )
        images.append(image)
        image_offset += len(image)
    for image in images:
        output.extend(image)
    output_path.write_bytes(output)


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    default_input = root / "apps" / "app" / "icons" / "icon.ico"
    default_output = default_input.with_name("icon-centered.ico")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", nargs="?", type=Path, default=default_input)
    parser.add_argument("output", nargs="?", type=Path, default=default_output)
    args = parser.parse_args()
    rewrite_ico(args.input, args.output)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
