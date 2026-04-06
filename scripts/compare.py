#!/usr/bin/env python3
import sys
import zlib
import os

MAGIC_BYTES = b"PlZ"


def decompress_sav_to_gvas(data: bytes) -> tuple[bytes, int]:
    uncompressed_len = int.from_bytes(data[0:4], byteorder="little")
    compressed_len = int.from_bytes(data[4:8], byteorder="little")
    magic_bytes = data[8:11]
    save_type = data[11]
    data_start_offset = 12

    if magic_bytes == b"CNK":
        uncompressed_len = int.from_bytes(data[12:16], byteorder="little")
        compressed_len = int.from_bytes(data[16:20], byteorder="little")
        magic_bytes = data[20:23]
        save_type = data[23]
        data_start_offset = 24
    if magic_bytes != MAGIC_BYTES:
        if (
            magic_bytes == b"\x00\x00\x00"
            and uncompressed_len == 0
            and compressed_len == 0
        ):
            raise Exception(
                f"not a compressed Palworld save, found too many null bytes, this is likely corrupted"
            )
        raise Exception(
            f"not a compressed Palworld save, found {magic_bytes!r} instead of {MAGIC_BYTES!r}"
        )

    if save_type not in [0x30, 0x31, 0x32]:
        raise Exception(f"unknown save type: {save_type}")

    if save_type not in [0x31, 0x32]:
        raise Exception(f"unhandled compression type: {save_type}")
    if save_type == 0x31:

        if compressed_len != len(data) - data_start_offset:
            raise Exception(f"incorrect compressed length: {compressed_len}")

    uncompressed_data = zlib.decompress(data[data_start_offset:])
    if save_type == 0x32:

        if compressed_len != len(uncompressed_data):
            raise Exception(f"incorrect compressed length: {compressed_len}")

        uncompressed_data = zlib.decompress(uncompressed_data)

    if uncompressed_len != len(uncompressed_data):
        raise Exception(f"incorrect uncompressed length: {uncompressed_len}")

    return uncompressed_data, save_type


def compress_gvas_to_sav(data: bytes, save_type: int) -> bytes:
    uncompressed_len = len(data)
    compressed_data = zlib.compress(data)
    compressed_len = len(compressed_data)
    if save_type == 0x32:
        compressed_data = zlib.compress(compressed_data)

    result = bytearray()
    result.extend(uncompressed_len.to_bytes(4, byteorder="little"))
    result.extend(compressed_len.to_bytes(4, byteorder="little"))
    result.extend(MAGIC_BYTES)
    result.extend(bytes([save_type]))
    result.extend(compressed_data)

    return bytes(result)


def compare_files(
    file1_data: bytes, file2_data: bytes
) -> tuple[bool, list[tuple[int, bytes, bytes]]]:
    differences = []
    identical = True

    min_length = min(len(file1_data), len(file2_data))

    for i in range(min_length):
        if file1_data[i] != file2_data[i]:
            differences.append((i, file1_data[i : i + 1], file2_data[i : i + 1]))
            identical = False

    if len(file1_data) != len(file2_data):
        identical = False
        differences.append(("Length difference", len(file1_data), len(file2_data)))

    return identical, differences


def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <save_file1> <save_file2>")
        sys.exit(1)

    file1_path = sys.argv[1]
    file2_path = sys.argv[2]

    if not os.path.exists(file1_path):
        print(f"Error: File {file1_path} does not exist")
        sys.exit(1)

    if not os.path.exists(file2_path):
        print(f"Error: File {file2_path} does not exist")
        sys.exit(1)

    try:

        with open(file1_path, "rb") as f:
            file1_data = f.read()

        with open(file2_path, "rb") as f:
            file2_data = f.read()

        print(f"Decompressing {file1_path}...")
        file1_decompressed, file1_save_type = decompress_sav_to_gvas(file1_data)

        print(f"Decompressing {file2_path}...")
        file2_decompressed, file2_save_type = decompress_sav_to_gvas(file2_data)

        file1_output = f"{file1_path}.decompressed"
        file2_output = f"{file2_path}.decompressed"

        with open(file1_output, "wb") as f:
            f.write(file1_decompressed)
        print(f"Decompressed data saved to {file1_output}")

        with open(file2_output, "wb") as f:
            f.write(file2_decompressed)
        print(f"Decompressed data saved to {file2_output}")

        print("\nComparing decompressed files...")
        identical, differences = compare_files(file1_decompressed, file2_decompressed)

        if identical:
            print("The files are identical after decompression.")
        else:
            print("The files are different after decompression.")
            print(f"File 1 size: {len(file1_decompressed)} bytes")
            print(f"File 2 size: {len(file2_decompressed)} bytes")

            if isinstance(differences[0][0], int):
                print(
                    f"\nFirst {min(10, len(differences))} differences (position, byte1, byte2):"
                )
                for i, (pos, byte1, byte2) in enumerate(differences[:10]):
                    print(f"  {i+1}. Position {pos}: {byte1.hex()} != {byte2.hex()}")

                if len(differences) > 10:
                    print(f"  ... and {len(differences) - 10} more differences")
            else:

                print(f"Length difference: {differences[0][1]} != {differences[0][2]}")

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
