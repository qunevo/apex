"""Independently verify saved XLSX cells against the unrounded JSON views."""
import hashlib
import json
import math
import re
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

root = Path(sys.argv[1]).resolve()
output = root / "outputs/scaling-20260924"
views = json.loads((root / "views.json").read_text())
checks = json.loads((output / "workbook_validation.json").read_text())
ns = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
workbook = output / "benchmark_views.xlsx"
checked = 0
with zipfile.ZipFile(workbook) as archive:
    shared = []
    if "xl/sharedStrings.xml" in archive.namelist():
        shared = ["".join(si.itertext()) for si in ET.fromstring(archive.read("xl/sharedStrings.xml"))]
    sheet_names = [sheet.attrib["name"] for sheet in ET.fromstring(archive.read("xl/workbook.xml")).find("m:sheets", ns)]
    assert sheet_names == [check["sheet"] for check in checks["sheets"]] + ["ReadMe"]
    for index, check in enumerate(checks["sheets"], 1):
        sheet = ET.fromstring(archive.read(f"xl/worksheets/sheet{index}.xml"))
        assert sheet.find("m:autoFilter", ns) is not None or sheet.find("m:tableParts", ns) is not None
        cells = {}
        for cell in sheet.findall(".//m:sheetData/m:row/m:c", ns):
            typ = cell.get("t")
            assert typ != "e", f"Excel error: {check['sheet']}!{cell.get('r')}"
            value = cell.findtext("m:v", default=None, namespaces=ns)
            if typ == "inlineStr":
                value = "".join(cell.find("m:is", ns).itertext())
            elif typ == "s":
                value = shared[int(value)]
            elif typ == "b":
                value = value == "1"
            elif value is not None and typ != "str":
                value = float(value)
            cells[cell.get("r")] = value
        start = int(re.search(r"\d+", check["table_range"]).group()) + 1
        for row_index, row in enumerate(views[check["view"]], start):
            for col_index, key in enumerate(check["columns"], 1):
                letters, number = "", col_index
                while number:
                    number, digit = divmod(number - 1, 26)
                    letters = chr(65 + digit) + letters
                cell_ref = f"{letters}{row_index}"
                actual, expected = cells.get(cell_ref), row[key]
                if isinstance(expected, (int, float)) and not isinstance(expected, bool):
                    assert isinstance(actual, (int, float)) and math.isclose(actual, expected, rel_tol=1e-14, abs_tol=1e-14), (check["sheet"], cell_ref, actual, expected)
                else:
                    assert actual == expected, (check["sheet"], cell_ref, actual, expected)
                checked += 1
    assert len([name for name in archive.namelist() if re.fullmatch(r"xl/tables/table\d+\.xml", name)]) == len(checks["sheets"])
report = dict(passed=True, saved_cells_verified=checked, sheets=sheet_names, workbook_sha256=hashlib.sha256(workbook.read_bytes()).hexdigest())
(output / "xlsx_validation.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report))
