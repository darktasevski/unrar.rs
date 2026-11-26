"""Generate a stored RAR5 multipart Windows read-only file with Python 3."""
from pathlib import Path
from zlib import crc32


def crc(data):
    return crc32(data).to_bytes(4, "little")


def block(fields):
    # All variable integers here fit in a byte.
    header = bytes([len(fields)]) + fields
    return crc(header) + header


parts = (b"first", b"second")
data_dir = Path(__file__).resolve().parents[2] / "data"
for i, part in enumerate(parts):
    main = bytes([1, 0, 1]) if i == 0 else bytes([1, 0, 3, 1])
    file_flags = 2 | (0x10 if i == 0 else 0x08)
    file_header = bytes([2, file_flags, len(part), 4, len(b"".join(parts)), 0x21])
    file_header += crc(part if i == 0 else b"".join(parts))
    file_header += bytes([0, 0, 4]) + b"file"
    archive = b"Rar!\x1a\x07\x01\x00" + block(main) + block(file_header) + part
    archive += block(bytes([5, 0, int(i == 0)]))
    (data_dir / f"windows-mode.part{i + 1}.rar").write_bytes(archive)
