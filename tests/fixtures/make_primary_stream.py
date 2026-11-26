"""Generate data/primary-stream.cbr with Python 3, using only stored RAR5 blocks."""
from pathlib import Path
from zlib import crc32


def crc(data):
    return crc32(data).to_bytes(4, "little")


def block(fields):
    # All sizes in this fixture fit in single-byte RAR variable integers.
    header = bytes([len(fields)]) + fields
    return crc(header) + header


def entry(kind, name, payload, extra=b""):
    flags = 2 if kind == 2 else 0x63  # inherited child stream with extra data
    fields = bytes([kind, flags]) + (bytes([len(extra)]) if extra else b"")
    fields += bytes([len(payload), 4, len(payload), 0x20]) + crc(payload)
    fields += bytes([0, 0, len(name)]) + name + extra
    return block(fields) + payload


stream_name = b":notes"
subdata = bytes([1 + len(stream_name), 7]) + stream_name
stream = entry(3, b"STM", b"stream bytes", subdata)
archive = (
    b"Rar!\x1a\x07\x01\x00"
    + block(bytes([1, 0, 0]))
    + entry(2, b"page", b"page bytes") + stream
    + entry(2, b"empty", b"") + stream
    + block(bytes([5, 0, 0]))
)
(Path(__file__).resolve().parents[2] / "data/primary-stream.cbr").write_bytes(archive)
