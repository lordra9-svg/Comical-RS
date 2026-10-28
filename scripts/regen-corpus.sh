#!/usr/bin/env bash
# Rebuild tests/corpus fixtures with Info-ZIP and 7-Zip.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pages="$root/tests/corpus/pages"
fixtures="$root/tests/corpus/fixtures"
password="corpus-secret"

command -v zip >/dev/null || { echo "zip (Info-ZIP) is required" >&2; exit 1; }
command -v 7z >/dev/null || { echo "7z is required" >&2; exit 1; }

mkdir -p "$pages/deep/path/here" "$fixtures"
python3 - <<PY
from pathlib import Path
pages = Path(r"$pages")
(pages / "a.png").write_bytes(b"\x89PNG\r\n\x1a\ninfozip-page-a" + b"A" * 4000)
(pages / "b.jpg").write_bytes(b"\xff\xd8\xff\xe0infozip-page-b" + b"B" * 4000)
(pages / "c.gif").write_bytes(b"GIF89ainfozip-page-c" + b"C" * 4000)
(pages / "uni-\u65e5\u672c\u8a9e.png").write_bytes(b"\x89PNG\r\n\x1a\nunicode-page" + b"U" * 2000)
(pages / "deep/path/here/nested.png").write_bytes(b"\x89PNG\r\n\x1a\nnested-page" + b"N" * 2000)
PY

rm -f "$fixtures"/*

(
  cd "$pages"
  zip -0 -X "$fixtures/infozip-store.cbz" a.png b.jpg c.gif
  zip -9 -X "$fixtures/infozip-deflate.cbz" a.png b.jpg c.gif
  zip -9 -X -P "$password" "$fixtures/infozip-zipcrypto.cbz" a.png b.jpg
  zip -9 -X "$fixtures/infozip-unicode-deep.cbz" "uni-日本語.png" deep/path/here/nested.png
  7z a -tzip -mx=9 -bso0 -bsp0 "$fixtures/sevenzip-zip-deflate.cbz" a.png b.jpg c.gif
  7z a -t7z -m0=LZMA2 -ms=off -mx=5 -bso0 -bsp0 "$fixtures/sevenzip-lzma2.cb7" a.png b.jpg c.gif
  7z a -t7z -m0=LZMA -ms=off -mx=5 -bso0 -bsp0 "$fixtures/sevenzip-lzma.cb7" a.png b.jpg c.gif
  7z a -t7z -m0=LZMA2 -ms=on -mx=5 -bso0 -bsp0 "$fixtures/sevenzip-solid.cb7" a.png b.jpg c.gif
  7z a -t7z -m0=LZMA2 -ms=off -p"$password" -mhe=off -mx=5 -bso0 -bsp0 "$fixtures/sevenzip-aes.cb7" a.png b.jpg
  7z a -t7z -m0=LZMA2 -ms=off -p"$password" -mhe=on -mx=5 -bso0 -bsp0 "$fixtures/sevenzip-aes-headers.cb7" a.png b.jpg
)

python3 - <<PY
import hashlib, json
from pathlib import Path
root = Path(r"$root")
pages_dir = root / "tests/corpus/pages"
fixtures = root / "tests/corpus/fixtures"
page_names = [
    "a.png", "b.jpg", "c.gif", "uni-\u65e5\u672c\u8a9e.png", "deep/path/here/nested.png",
]
pages = {n: (pages_dir / n).read_bytes() for n in page_names}
deflate = (fixtures / "infozip-deflate.cbz").read_bytes()
(fixtures / "truncated-zip.cbz").write_bytes(deflate[: max(32, len(deflate) // 4)])
lzma2 = (fixtures / "sevenzip-lzma2.cb7").read_bytes()
(fixtures / "truncated-7z.cb7").write_bytes(lzma2[: max(32, len(lzma2) // 4)])

cases = [
    ("infozip-store.cbz", "infozip", False, ["a.png", "b.jpg", "c.gif"], "ok"),
    ("infozip-deflate.cbz", "infozip", False, ["a.png", "b.jpg", "c.gif"], "ok"),
    ("sevenzip-zip-deflate.cbz", "sevenzip", False, ["a.png", "b.jpg", "c.gif"], "ok"),
    ("sevenzip-lzma2.cb7", "sevenzip", False, ["a.png", "b.jpg", "c.gif"], "ok"),
    ("sevenzip-lzma.cb7", "sevenzip", False, ["a.png", "b.jpg", "c.gif"], "ok"),
    ("sevenzip-solid.cb7", "sevenzip", False, ["a.png", "b.jpg", "c.gif"], "ok"),
    ("infozip-unicode-deep.cbz", "infozip", False, ["deep/path/here/nested.png", "uni-\u65e5\u672c\u8a9e.png"], "ok"),
    ("infozip-zipcrypto.cbz", "infozip", True, ["a.png", "b.jpg"], "ok"),
    ("sevenzip-aes.cb7", "sevenzip", True, ["a.png", "b.jpg"], "ok"),
    ("sevenzip-aes-headers.cb7", "sevenzip", True, ["a.png", "b.jpg"], "ok"),
    ("truncated-zip.cbz", "truncated", False, [], "open_error"),
    ("truncated-7z.cb7", "truncated", False, [], "open_error"),
]
manifest = {
    "password": "corpus-secret",
    "pages": {
        n: {"size": len(b), "sha256": hashlib.sha256(b).hexdigest()}
        for n, b in pages.items()
    },
    "cases": [
        {
            "fixture": fixture,
            "writer": writer,
            "password": needs_password,
            "pages": page_list,
            "expect": expect,
        }
        for fixture, writer, needs_password, page_list, expect in cases
    ],
}
(root / "tests/corpus/manifest.json").write_text(
    json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
print("regenerated", len(list(fixtures.iterdir())), "fixtures")
PY

echo "Run: cargo test -p comical-archive --test corpus"
